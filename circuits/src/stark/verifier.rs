use std::any::type_name;

use anyhow::{ensure, Result};
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::types::Field;
use plonky2::fri::verifier::verify_fri_proof;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::{GenericConfig, Hasher};
use plonky2::plonk::plonk_common::reduce_with_powers;

use super::config::StarkConfig;
use super::constraint_consumer::ConstraintConsumer;
use super::cross_table_lookup::{verify_cross_table_lookups, CtlCheckVars};
use super::ola_stark::{OlaStark, Table, NUM_TABLES};
use super::permutation::{GrandProductChallenge, PermutationCheckVars};
use super::proof::{
    AllProof, AllProofChallenges, PublicValues, StarkOpeningSet, StarkProof, StarkProofChallenges,
};
use super::stark::Stark;
use super::vanishing_poly::eval_vanishing_poly;
use super::vars::StarkEvaluationVars;
use crate::builtins::bitwise::bitwise_stark::BitwiseStark;
use crate::builtins::cmp::cmp_stark::CmpStark;
use crate::builtins::poseidon::poseidon_chunk_stark::PoseidonChunkStark;
use crate::builtins::poseidon::poseidon_stark::PoseidonStark;
use crate::builtins::rangecheck::rangecheck_stark::RangeCheckStark;
use crate::builtins::sccall::sccall_stark::SCCallStark;
use crate::builtins::storage::storage_access_stark::StorageAccessStark;
// use crate::builtins::tape::tape_stark::TapeStark;
use crate::cpu::cpu_stark::CpuStark;
use crate::memory::memory_stark::MemoryStark;
use crate::program::prog_chunk_stark::ProgChunkStark;
use crate::program::program_stark::ProgramStark;

pub fn verify_proof<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize>(
    ola_stark: OlaStark<F, D>,
    all_proof: AllProof<F, C, D>,
    config: &StarkConfig,
) -> Result<()>
where
    [(); C::Hasher::HASH_SIZE]:,
    [(); CpuStark::<F, D>::COLUMNS]:,
    [(); MemoryStark::<F, D>::COLUMNS]:,
    [(); BitwiseStark::<F, D>::COLUMNS]:,
    [(); CmpStark::<F, D>::COLUMNS]:,
    [(); RangeCheckStark::<F, D>::COLUMNS]:,
    [(); PoseidonStark::<F, D>::COLUMNS]:,
    [(); PoseidonChunkStark::<F, D>::COLUMNS]:,
    [(); StorageAccessStark::<F, D>::COLUMNS]:,
    // [(); TapeStark::<F, D>::COLUMNS]:,
    [(); SCCallStark::<F, D>::COLUMNS]:,
    [(); ProgramStark::<F, D>::COLUMNS]:,
    [(); ProgChunkStark::<F, D>::COLUMNS]:,
{
    let AllProofChallenges {
        stark_challenges,
        ctl_challenges,
    } = all_proof.get_challenges(&ola_stark, config);

    let nums_permutation_zs = ola_stark.nums_permutation_zs(config);

    let OlaStark {
        cpu_stark,
        memory_stark,
        mut bitwise_stark,
        cmp_stark,
        rangecheck_stark,
        poseidon_stark,
        poseidon_chunk_stark,
        storage_access_stark,
        tape_stark,
        sccall_stark,
        mut program_stark,
        prog_chunk_stark,
        cross_table_lookups,
    } = ola_stark;

    if bitwise_stark.get_compress_challenge().is_none() {
        bitwise_stark
            .set_compress_challenge(all_proof.compress_challenges[Table::Bitwise as usize])
            .unwrap();
    }
    if program_stark.get_compress_challenge().is_none() {
        program_stark
            .set_compress_challenge(all_proof.compress_challenges[Table::Program as usize])
            .unwrap();
    }

    let ctl_vars_per_table = CtlCheckVars::from_proofs(
        &all_proof.stark_proofs,
        &cross_table_lookups,
        &ctl_challenges,
        &nums_permutation_zs,
    );

    verify_stark_proof_with_challenges(
        cpu_stark,
        &all_proof.stark_proofs[Table::Cpu as usize],
        &stark_challenges[Table::Cpu as usize],
        &ctl_vars_per_table[Table::Cpu as usize],
        config,
    )?;
    verify_stark_proof_with_challenges(
        memory_stark,
        &all_proof.stark_proofs[Table::Memory as usize],
        &stark_challenges[Table::Memory as usize],
        &ctl_vars_per_table[Table::Memory as usize],
        config,
    )?;
    verify_stark_proof_with_challenges(
        bitwise_stark,
        &all_proof.stark_proofs[Table::Bitwise as usize],
        &stark_challenges[Table::Bitwise as usize],
        &ctl_vars_per_table[Table::Bitwise as usize],
        config,
    )?;

    verify_stark_proof_with_challenges(
        cmp_stark,
        &all_proof.stark_proofs[Table::Cmp as usize],
        &stark_challenges[Table::Cmp as usize],
        &ctl_vars_per_table[Table::Cmp as usize],
        config,
    )?;

    verify_stark_proof_with_challenges(
        rangecheck_stark,
        &all_proof.stark_proofs[Table::RangeCheck as usize],
        &stark_challenges[Table::RangeCheck as usize],
        &ctl_vars_per_table[Table::RangeCheck as usize],
        config,
    )?;

    verify_stark_proof_with_challenges(
        poseidon_stark,
        &all_proof.stark_proofs[Table::Poseidon as usize],
        &stark_challenges[Table::Poseidon as usize],
        &ctl_vars_per_table[Table::Poseidon as usize],
        config,
    )?;

    verify_stark_proof_with_challenges(
        poseidon_chunk_stark,
        &all_proof.stark_proofs[Table::PoseidonChunk as usize],
        &stark_challenges[Table::PoseidonChunk as usize],
        &ctl_vars_per_table[Table::PoseidonChunk as usize],
        config,
    )?;

    verify_stark_proof_with_challenges(
        storage_access_stark,
        &all_proof.stark_proofs[Table::StorageAccess as usize],
        &stark_challenges[Table::StorageAccess as usize],
        &ctl_vars_per_table[Table::StorageAccess as usize],
        config,
    )?;

    verify_stark_proof_with_challenges(
        tape_stark,
        &all_proof.stark_proofs[Table::Tape as usize],
        &stark_challenges[Table::Tape as usize],
        &ctl_vars_per_table[Table::Tape as usize],
        config,
    )?;

    verify_stark_proof_with_challenges(
        sccall_stark,
        &all_proof.stark_proofs[Table::SCCall as usize],
        &stark_challenges[Table::SCCall as usize],
        &ctl_vars_per_table[Table::SCCall as usize],
        config,
    )?;

    verify_stark_proof_with_challenges(
        program_stark,
        &all_proof.stark_proofs[Table::Program as usize],
        &stark_challenges[Table::Program as usize],
        &ctl_vars_per_table[Table::Program as usize],
        config,
    )?;

    verify_stark_proof_with_challenges(
        prog_chunk_stark,
        &all_proof.stark_proofs[Table::ProgChunk as usize],
        &stark_challenges[Table::ProgChunk as usize],
        &ctl_vars_per_table[Table::ProgChunk as usize],
        config,
    )?;

    // TODO:
    // let public_values = all_proof.public_values;
    let extra_looking_products = vec![vec![F::ONE; config.num_challenges]; NUM_TABLES];
    // extra_looking_products.push(Vec::new());
    // for c in 0..config.num_challenges {
    //     extra_looking_products[Table::StorageAccess as usize].push(
    //         get_storagehash_extra_looking_products(&public_values,
    // ctl_challenges.challenges[c]),     );
    // }

    verify_cross_table_lookups::<F, C, D>(
        cross_table_lookups,
        all_proof.stark_proofs.map(|p| p.openings.ctl_zs_last),
        extra_looking_products,
        config,
    )
}

pub(crate) fn get_storagehash_extra_looking_products<F, const D: usize>(
    _public_values: &PublicValues,
    _challenge: GrandProductChallenge<F>,
) -> F
where
    F: RichField + Extendable<D>,
{
    let prod = F::ONE;
    prod
}

pub(crate) fn verify_stark_proof_with_challenges<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    S: Stark<F, D>,
    const D: usize,
>(
    stark: S,
    proof: &StarkProof<F, C, D>,
    challenges: &StarkProofChallenges<F, D>,
    ctl_vars: &[CtlCheckVars<F, F::Extension, F::Extension, D>],
    config: &StarkConfig,
) -> Result<()>
where
    [(); S::COLUMNS]:,
    [(); C::Hasher::HASH_SIZE]:,
{
    validate_proof_shape(&stark, proof, config, ctl_vars.len())?;
    let StarkOpeningSet {
        local_values,
        next_values,
        permutation_ctl_zs,
        permutation_ctl_zs_next,
        ctl_zs_last,
        quotient_polys,
    } = &proof.openings;
    let vars = StarkEvaluationVars {
        local_values: &local_values.to_vec().try_into().unwrap(),
        next_values: &next_values.to_vec().try_into().unwrap(),
    };

    let degree_bits = proof.recover_degree_bits(config);
    let (l_0, l_last) = eval_l_0_and_l_last(degree_bits, challenges.stark_zeta);
    let last = F::primitive_root_of_unity(degree_bits).inverse();
    let z_last = challenges.stark_zeta - last.into();
    let mut consumer = ConstraintConsumer::<F::Extension>::new(
        challenges
            .stark_alphas
            .iter()
            .map(|&alpha| F::Extension::from_basefield(alpha))
            .collect::<Vec<_>>(),
        z_last,
        l_0,
        l_last,
    );
    let num_permutation_zs = stark.num_permutation_batches(config);
    let permutation_data = stark.uses_permutation_args().then(|| PermutationCheckVars {
        local_zs: permutation_ctl_zs[..num_permutation_zs].to_vec(),
        next_zs: permutation_ctl_zs_next[..num_permutation_zs].to_vec(),
        permutation_challenge_sets: challenges.permutation_challenge_sets.clone().unwrap(),
    });
    eval_vanishing_poly::<F, F::Extension, F::Extension, C, S, D, D>(
        &stark,
        config,
        vars,
        permutation_data,
        ctl_vars,
        &mut consumer,
    );
    let vanishing_polys_zeta = consumer.accumulators();

    // Check each polynomial identity, of the form `vanishing(x) = Z_H(x)
    // quotient(x)`, at zeta.
    let zeta_pow_deg = challenges.stark_zeta.exp_power_of_2(degree_bits);
    let z_h_zeta = zeta_pow_deg - F::Extension::ONE;
    // `quotient_polys_zeta` holds `num_challenges * quotient_degree_factor`
    // evaluations. Each chunk of `quotient_degree_factor` holds the evaluations
    // of `t_0(zeta),...,t_{quotient_degree_factor-1}(zeta)` where the "real"
    // quotient polynomial is `t(X) = t_0(X) + t_1(X)*X^n + t_2(X)*X^{2n} + ...`.
    // So to reconstruct `t(zeta)` we can compute `reduce_with_powers(chunk,
    // zeta^n)` for each `quotient_degree_factor`-sized chunk of the original
    // evaluations.
    for (i, chunk) in quotient_polys
        .chunks(stark.quotient_degree_factor())
        .enumerate()
    {
        ensure!(
            vanishing_polys_zeta[i] == z_h_zeta * reduce_with_powers(chunk, zeta_pow_deg),
            "Mismatch between evaluation and opening of quotient polynomial in {}",
            type_name::<S>()
        );
    }

    let merkle_caps = vec![
        proof.trace_cap.clone(),
        proof.permutation_ctl_zs_cap.clone(),
        proof.quotient_polys_cap.clone(),
    ];

    verify_fri_proof::<F, C, D>(
        &stark.fri_instance(
            challenges.stark_zeta,
            F::primitive_root_of_unity(degree_bits),
            degree_bits,
            ctl_zs_last.len(),
            config,
        ),
        &proof.openings.to_fri_openings(),
        &challenges.fri_challenges,
        &merkle_caps,
        &proof.opening_proof,
        &config.fri_params(degree_bits),
    )?;

    Ok(())
}

fn validate_proof_shape<F, C, S, const D: usize>(
    stark: &S,
    proof: &StarkProof<F, C, D>,
    config: &StarkConfig,
    num_ctl_zs: usize,
) -> anyhow::Result<()>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    S: Stark<F, D>,
    [(); S::COLUMNS]:,
    [(); C::Hasher::HASH_SIZE]:,
{
    let StarkProof {
        trace_cap,
        permutation_ctl_zs_cap,
        quotient_polys_cap,
        openings,
        // The shape of the opening proof will be checked in the FRI verifier (see
        // validate_fri_proof_shape), so we ignore it here.
        opening_proof: _,
    } = proof;

    let StarkOpeningSet {
        local_values,
        next_values,
        permutation_ctl_zs,
        permutation_ctl_zs_next,
        ctl_zs_last,
        quotient_polys,
    } = openings;

    let degree_bits = proof.recover_degree_bits(config);
    let fri_params = config.fri_params(degree_bits);
    let cap_height = fri_params.config.cap_height;
    let num_zs = num_ctl_zs + stark.num_permutation_batches(config);

    ensure!(trace_cap.height() == cap_height);
    ensure!(permutation_ctl_zs_cap.height() == cap_height);
    ensure!(quotient_polys_cap.height() == cap_height);

    ensure!(local_values.len() == S::COLUMNS);
    ensure!(next_values.len() == S::COLUMNS);
    ensure!(permutation_ctl_zs.len() == num_zs);
    ensure!(permutation_ctl_zs_next.len() == num_zs);
    ensure!(ctl_zs_last.len() == num_ctl_zs);
    ensure!(quotient_polys.len() == stark.num_quotient_polys(config));

    Ok(())
}

/// Evaluate the Lagrange polynomials `L_0` and `L_(n-1)` at a point `x`.
/// `L_0(x) = (x^n - 1)/(n * (x - 1))`
/// `L_(n-1)(x) = (x^n - 1)/(n * (g * x - 1))`, with `g` the first element of
/// the subgroup.
fn eval_l_0_and_l_last<F: Field>(log_n: usize, x: F) -> (F, F) {
    let n = F::from_canonical_usize(1 << log_n);
    let g = F::primitive_root_of_unity(log_n);
    let z_x = x.exp_power_of_2(log_n) - F::ONE;
    let invs = F::batch_multiplicative_inverse(&[n * (x - F::ONE), n * (g * x - F::ONE)]);

    (z_x * invs[0], z_x * invs[1])
}

#[cfg(test)]
mod tests {
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::polynomial::PolynomialValues;
    use plonky2::field::types::Field;

    use crate::stark::verifier::eval_l_0_and_l_last;

    #[test]
    fn test_eval_l_0_and_l_last() {
        type F = GoldilocksField;
        let log_n = 5;
        let n = 1 << log_n;

        let x = F::rand(); // challenge point
        let expected_l_first_x = PolynomialValues::selector(n, 0).ifft().eval(x);
        let expected_l_last_x = PolynomialValues::selector(n, n - 1).ifft().eval(x);

        let (l_first_x, l_last_x) = eval_l_0_and_l_last(log_n, x);
        assert_eq!(l_first_x, expected_l_first_x);
        assert_eq!(l_last_x, expected_l_last_x);
    }
}
