use std::mem::MaybeUninit;
use core::slice;

use maybe_rayon::*;
use plonky2_field::cfft::uninit_vector;
use plonky2_util::log2_strict;
use serde::{Deserialize, Serialize};

use crate::hash::concurrent;
use crate::hash::hash_types::RichField;
use crate::hash::merkle_proofs::MerkleProof;
use crate::plonk::config::GenericHashOut;
use crate::plonk::config::Hasher;



/// The Merkle cap of height `h` of a Merkle tree is the `h`-th layer (from the
/// root) of the tree. It can be used in place of the root to verify Merkle
/// paths, which are `h` elements shorter.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(bound = "")]
pub struct MerkleCap<F: RichField, H: Hasher<F>>(pub Vec<H::Hash>);

impl<F: RichField, H: Hasher<F>> MerkleCap<F, H> {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn height(&self) -> usize {
        log2_strict(self.len())
    }

    pub fn flatten(&self) -> Vec<F> {
        self.0.iter().flat_map(|&h| h.to_vec()).collect()
    }
}

#[derive(Clone, Debug)]
pub struct MerkleTree<F: RichField, H: Hasher<F>> {
    /// The data in the leaves of the Merkle tree.
    pub leaves: Vec<Vec<F>>,

    /// The digests in the tree. Consists of `cap.len()` sub-trees, each
    /// corresponding to one element in `cap`. Each subtree is contiguous
    /// and located at `digests[digests.len() / cap.len() * i..digests.len()
    /// / cap.len() * (i + 1)]`. Within each subtree, siblings are stored
    /// next to each other. The layout is, left_child_subtree ||
    /// left_child_digest || right_child_digest || right_child_subtree, where
    /// left_child_digest and right_child_digest are H::Hash and
    /// left_child_subtree and right_child_subtree recurse. Observe that the
    /// digest of a node is stored by its _parent_. Consequently, the
    /// digests of the roots are not stored here (they can be found in `cap`).
    pub digests: Vec<H::Hash>,

    /// The Merkle cap.
    pub cap: MerkleCap<F, H>,
}

fn capacity_up_to_mut<T>(v: &mut Vec<T>, len: usize) -> &mut [MaybeUninit<T>] {
    assert!(v.capacity() >= len);
    let v_ptr = v.as_mut_ptr().cast::<MaybeUninit<T>>();
    unsafe {
        // SAFETY: `v_ptr` is a valid pointer to a buffer of length at least `len`. Upon
        // return, the lifetime will be bound to that of `v`. The underlying
        // memory will not be deallocated as we hold the sole mutable reference
        // to `v`. The contents of the slice may be uninitialized, but the
        // `MaybeUninit` makes it safe.
        slice::from_raw_parts_mut(v_ptr, len)
    }
}

fn fill_subtree<F: RichField, H: Hasher<F>>(
    digests_buf: &mut [MaybeUninit<H::Hash>],
    leaves: &[Vec<F>],
) -> H::Hash
where
    [(); H::HASH_SIZE]:,
{
    assert_eq!(leaves.len(), digests_buf.len() / 2 + 1);
    if digests_buf.is_empty() {
        H::hash_or_noop(&leaves[0])
    } else {
        // Layout is: left recursive output || left child digest
        //             || right child digest || right recursive output.
        // Split `digests_buf` into the two recursive outputs (slices) and two child
        // digests (references).

        let (left_digests_buf, right_digests_buf) = digests_buf.split_at_mut(digests_buf.len() / 2);
        let (left_digest_mem, left_digests_buf) = left_digests_buf.split_last_mut().unwrap();
        let (right_digest_mem, right_digests_buf) = right_digests_buf.split_first_mut().unwrap();
        // Split `leaves` between both children.
        let (left_leaves, right_leaves) = leaves.split_at(leaves.len() / 2);

        let (left_digest, right_digest) = maybe_rayon::join(
            || fill_subtree::<F, H>(left_digests_buf, left_leaves),
            || fill_subtree::<F, H>(right_digests_buf, right_leaves),
        );

        left_digest_mem.write(left_digest);
        right_digest_mem.write(right_digest);
        H::two_to_one(left_digest, right_digest)
    }
}

fn fill_digests_buf<F: RichField, H: Hasher<F>>(
    digests_buf: &mut [MaybeUninit<H::Hash>],
    cap_buf: &mut [MaybeUninit<H::Hash>],
    leaves: &[Vec<F>],
    cap_height: usize,
) where
    [(); H::HASH_SIZE]:,
{
    // Special case of a tree that's all cap. The usual case will panic because
    // we'll try to split an empty slice into chunks of `0`. (We would not need
    // this if there was a way to split into `blah` chunks as opposed to chunks
    // _of_ `blah`.)
    if digests_buf.is_empty() {
        debug_assert_eq!(cap_buf.len(), leaves[0].len());
        cap_buf
            .par_iter_mut()
            .zip(leaves)
            .for_each(|(cap_buf, leaf)| {
                cap_buf.write(H::hash_or_noop(leaf));
            });
        return;
    }

    let subtree_digests_len = digests_buf.len() >> cap_height;
    let subtree_leaves_len = leaves[0].len() >> cap_height;
    let digests_chunks = digests_buf.par_chunks_exact_mut(subtree_digests_len);
    let leaves_chunks = leaves.par_chunks_exact(subtree_leaves_len);

    assert_eq!(digests_chunks.len(), cap_buf.len());
    assert_eq!(digests_chunks.len(), leaves_chunks.len());

    digests_chunks.zip(cap_buf).zip(leaves_chunks).for_each(
        |((subtree_digests, subtree_cap), subtree_leaves)| {
            // We have `1 << cap_height` sub-trees, one for each entry in `cap`. They are
            // totally independent, so we schedule one task for each.
            // `digests_buf` and `leaves` are split into `1 << cap_height`
            // slices, one for each sub-tree.
            subtree_cap.write(fill_subtree::<F, H>(subtree_digests, subtree_leaves));
        },
    );
}

impl<F: RichField, H: Hasher<F>> MerkleTree<F, H> {
    pub fn new(leaves: Vec<Vec<F>>, cap_height: usize) -> Self
    where
        [(); H::HASH_SIZE]:,
    {   // TODO: add time
        // let now = std::time::Instant::now();
        let log2_leaves_len = log2_strict(leaves.len());
        assert!(
            cap_height <= log2_leaves_len,
            "cap height should be at most log2(leaves.len())"
        );
        let num_digests = 2 * (leaves.len() - (1 << cap_height));
        let mut digests = Vec::with_capacity(num_digests);
        
        // len_cap = 2
        let len_cap = 1 << cap_height;
        let mut cap = Vec::with_capacity(len_cap);

        let digests_buf = capacity_up_to_mut(&mut digests, num_digests);
        let cap_buf = capacity_up_to_mut(&mut cap, len_cap);
        fill_digests_buf::<F, H>(digests_buf, cap_buf, &leaves[..], cap_height);

        unsafe {
            // SAFETY: `fill_digests_buf` and `cap` initialized the spare capacity up to
            // `num_digests` and `len_cap`, resp.
            digests.set_len(num_digests);
            cap.set_len(len_cap);
        }

        // println!("build olavm merkle tree time: {:?}", now.elapsed());

        // let tree = Self::new1(leaves.clone(), cap_height);

        // let by = digests[14].to_bytes();
        // for (i, d) in tree.digests.iter().enumerate() {
        //     if by == d.to_bytes() {
        //         println!("{}", i);
        //     }
        // }

        // for (i, (d1, d2)) in digests.iter().zip(tree.digests).enumerate() {
        //     if d1.to_bytes() != d2.to_bytes() {
        //         println!("not equal!!!!!!!!!!!!!!!!!{}", i);
        //     }
        // }

        // tree

        Self {
            leaves,
            digests,
            cap: MerkleCap(cap),
        }
    }

    // pub fn new1(leaves: Vec<Vec<F>>, cap_height: usize) -> Self
    // where
    //     [(); H::HASH_SIZE]:,
    // {   
    //     // TODO: add time
    //     let now = std::time::Instant::now();
        
    //     let leaves_len = leaves.len();

    //     // assert!(
    //     //     leaves_len >= 2,
    //     //     "too few leaves"
    //     // );

    //     // assert!(
    //     //     leaves_len.is_power_of_two(),
    //     //     "number of leaves not power of two"
    //     // );

    //     let mut row_hashes = unsafe { uninit_vector::<H::Hash>(leaves_len) };

    //     batch_iter_mut!(
    //         &mut row_hashes,
    //         128, // min batch size
    //         |batch: &mut [H::Hash], batch_offset: usize| {
    //             let mut row_buf = vec![F::ZERO; leaves[0].len()];
    //             for (i, row_hash) in batch.iter_mut().enumerate() {
    //                 let row_idx = i + batch_offset;
    //                 for (j, value) in (0..leaves[0].len()).into_iter().zip(row_buf.iter_mut()) {
    //                     *value = leaves[row_idx][j];
    //                 }
    //                 *row_hash = H::hash_or_noop(&row_buf);
    //             }
    //         }
    //     );

    //     #[cfg(not(feature = "parallel"))]
    //     let nodes = build_merkle_nodes::<F, H>(&row_hashes);

    //     #[cfg(feature = "parallel")]
    //     let nodes = if leaves_len <= concurrent::MIN_CONCURRENT_LEAVES {
    //         build_merkle_nodes::<F, H>(&row_hashes)
    //     } else {
    //         concurrent::build_merkle_nodes::<F, H>(&row_hashes)
    //     };

    //     if leaves.len() == 1 << 23 && leaves[0].len() == 76 {
    //         println!("build winterfell merkle tree time: {:?}", now.elapsed());
    //     }


    //     // add time
    //     let now = std::time::Instant::now();

    //     let num_digests = 2 * (leaves_len - (1 << cap_height));
    //     let mut digests = unsafe { uninit_vector::<H::Hash>(num_digests) };
    //     let len_cap = 1 << cap_height;
    //     let mut cap = unsafe { uninit_vector::<H::Hash>(len_cap) };

    //     if len_cap == leaves_len {
    //         for i in 0..len_cap {
    //             cap[i] = row_hashes[i];
    //         }
    //     } else {
    //         for i in 0..len_cap {
    //             cap[i] = nodes[i + len_cap];
    //         }
    //     }
        

    //     let tree_height_sub_1 = log2_strict(leaves_len);
    //     let num_layers =  tree_height_sub_1 - cap_height;
    //     let num_sub_tree_leaves = 1 << num_layers;
    //     let tree_len = num_digests >> cap_height;
    //     let num_trees = 1 << cap_height;

    //     // digests.as_mut_slice().par_chunks_exact_mut(tree_len).enumerate().for_each(|(i, sub_digests)| {
    //     if num_digests > 0 {
    //         for i in 0..num_trees {
    //             for pair_idx in (0..num_sub_tree_leaves).step_by(2) {
    //                 let siblings_index = pair_idx;
    //                 let sibling_index = siblings_index << 1;
    //                 digests[tree_len * i + sibling_index] = row_hashes[num_sub_tree_leaves * i + pair_idx];
    //                 digests[tree_len * i + sibling_index + 1] = row_hashes[num_sub_tree_leaves * i + pair_idx + 1];
    //             }
    
    //             for layer in 1..num_layers {
    //                 let num_layer_nodes = num_sub_tree_leaves >> layer;
    //                 for pair_idx in (0..num_layer_nodes).step_by(2) {
    //                     let siblings_index = (pair_idx << layer) + (1 << layer) - 1;
    //                     let sibling_index = siblings_index << 1;
    //                     let n_idx = (1 << (tree_height_sub_1 - layer)) + num_layer_nodes * i + pair_idx;
    //                     digests[tree_len * i + sibling_index] = nodes[n_idx];
    //                     digests[tree_len * i + sibling_index + 1] = nodes[n_idx + 1];
    //                 }
    //             }
    //         }
    //     }

    //     if leaves.len() == 1 << 23 && leaves[0].len() == 76 {
    //         println!("winterfell to olavm merkle tree time: {:?}", now.elapsed());
    //     }

        

    //     Self {
    //         leaves,
    //         digests,
    //         cap: MerkleCap(cap),
    //     }
    // }

    pub fn get(&self, i: usize) -> &[F] {
        &self.leaves[i]
    }

    /// Create a Merkle proof from a leaf index.
    pub fn prove(&self, leaf_index: usize) -> MerkleProof<F, H> {
        let cap_height = log2_strict(self.cap.len());
        let num_layers = log2_strict(self.leaves.len()) - cap_height;
        // leaf_index <= 2^{cap_height + num_layers} = leaves.len()
        debug_assert_eq!(leaf_index >> (cap_height + num_layers), 0);

        if leaf_index == 240 {
            println!("sss");
        }

        // 2^num_layers表示子树的叶子节点数
        // tree_index表示第几个子树，0 ~ 2^c - 1
        // tree_len表示每棵子树的digests节点数
        // digest_tree表示这棵子树在digests中的下标范围

        // leaf_index = 2, digests = 28, leaves = 16, cap = 2, cap_height = 1, num_layers = 3
        // digest_tree = digests[14..28]
        let digest_tree = {
            let tree_index = leaf_index >> num_layers;
            let tree_len = self.digests.len() >> cap_height;
            &self.digests[tree_len * tree_index..tree_len * (tree_index + 1)]
        };

        // Mask out high bits to get the index within the sub-tree.
        // leaf_index 表示子树下标        
        // 1 << num_layers - 1 = 111111111 共 num_layers 个1
        // pair_index ~ [0, (1 << num_layers) - 1]
        // leaf_index = 4, num_layers = 3
        // pair_index = 4
        // digest_tree = digests[14..28]

        // pair_index 表示当前leaf_index在子树中的叶子节点下标
        let mut pair_index = leaf_index & ((1 << num_layers) - 1);
        let siblings = (0..num_layers)
            .into_iter()
            .map(|i| {
                // i = 0
                // parity = 0
                // pair_index = 2
                // siblings_index = 4
                // sibling_index = 9

                // i = 1
                // parity = 0
                // pair_index = 1
                // siblings_index = 5
                // sibling_index = 11

                // i = 2
                // parity = 1
                // pair_index = 0
                // siblings_index = 3
                // sibling_index = 6
                
                // parity表示当前查询节点在左边还是右边
                let parity = pair_index & 1;
                // pair_index表示当前查询节点在上一层中的下标
                pair_index >>= 1;

                // The layers' data is interleaved as follows:
                // [layer 0, layer 1, layer 0, layer 2, layer 0, layer 1, layer 0, layer 3,
                // ...]. Each of the above is a pair of siblings.
                // `pair_index` is the index of the pair within layer `i`.
                // The index of that the pair within `digests` is
                // `pair_index * 2 ** (i + 1) + (2 ** i - 1)`.
                // siblings_index表示什么？
                let siblings_index = (pair_index << (i + 1)) + (1 << i) - 1;
                // We have an index for the _pair_, but we want the index of the _sibling_.
                // Double the pair index to get the index of the left sibling. Conditionally add
                // `1` if we are to retrieve the right sibling.
                let sibling_index = 2 * siblings_index + (1 - parity);
                digest_tree[sibling_index]
            })
            .collect();

        MerkleProof { siblings }
    }
}

pub fn build_merkle_nodes<F: RichField, H: Hasher<F>>(leaves: &[H::Hash]) -> Vec<H::Hash>
where
    [(); H::HASH_SIZE]: {
    let n = leaves.len() / 2;
    // create un-initialized array to hold all intermediate nodes
    let mut nodes = unsafe { uninit_vector::<H::Hash>(2 * n) };
    nodes[0] = H::zero_hash();

    // re-interpret leaves as an array of two leaves fused together
    let two_leaves = unsafe { slice::from_raw_parts(leaves.as_ptr() as *const [H::Hash; 2], n) };

    // build first row of internal nodes (parents of leaves)
    for (i, j) in (0..n).zip(n..nodes.len()) {
        nodes[j] = H::two_to_one(two_leaves[i][0], two_leaves[i][1]);
    }

    // re-interpret nodes as an array of two nodes fused together
    let two_nodes = unsafe { slice::from_raw_parts(nodes.as_ptr() as *const [H::Hash; 2], n) };

    // calculate all other tree nodes
    for i in (1..n).rev() {
        nodes[i] = H::two_to_one(two_nodes[i][0], two_nodes[i][1]);
    }

    nodes
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use plonky2_field::extension::Extendable;

    use super::*;
    use crate::hash::merkle_proofs::verify_merkle_proof_to_cap;
    use crate::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

    fn random_data<F: RichField>(n: usize, k: usize) -> Vec<Vec<F>> {
        (0..n).map(|_| F::rand_vec(k)).collect()
    }

    fn verify_all_leaves<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize>(
        leaves: Vec<Vec<F>>,
        cap_height: usize,
    ) -> Result<()>
    where
        [(); C::Hasher::HASH_SIZE]:,
    {
        let tree = MerkleTree::<F, C::Hasher>::new(leaves.clone(), cap_height);
        for (i, leaf) in leaves.into_iter().enumerate() {
            let proof = tree.prove(i);
            verify_merkle_proof_to_cap(leaf, i, &tree.cap, &proof)?;
        }
        Ok(())
    }

    #[test]
    #[should_panic]
    fn test_cap_height_too_big() {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;

        let log_n = 8;
        let cap_height = log_n + 1; // Should panic if `cap_height > len_n`.

        let leaves = random_data::<F>(1 << log_n, 7);
        let _ = MerkleTree::<F, <C as GenericConfig<D>>::Hasher>::new(leaves, cap_height);
    }

    #[test]
    fn test_cap_height_eq_log2_len() -> Result<()> {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;

        let log_n = 8;
        let n = 1 << log_n;
        let leaves = random_data::<F>(n, 7);

        verify_all_leaves::<F, C, D>(leaves, log_n)?;

        Ok(())
    }

    #[test]
    fn test_merkle_trees() -> Result<()> {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;

        let log_n = 8;
        let n = 1 << log_n;
        let leaves = random_data::<F>(n, 7);

        verify_all_leaves::<F, C, D>(leaves, 1)?;

        Ok(())
    }
}
