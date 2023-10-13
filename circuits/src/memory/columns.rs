use std::collections::BTreeMap;

// Memory Trace.
// ┌───────┬──────┬─────┬────┬──────────┬───────┬───────────┬───────────────┬──────────┬────────────────┬
// │ is_rw │ addr │ clk │ op │ is_write │ value │ diff_addr │ diff_addr_inv │
// diff_clk │ diff_addr_cond │
// └───────┴──────┴─────┴────┴──────────┴───────┴───────────┴───────────────┴──────────┴────────────────┴
// ┬────────────────────────┬───────────────────┬────────────────┬─────────────────┬──────────────┬──────────┬───────────────────┐
// │ filter_looked_for_main │ rw_addr_unchanged │ region_prophet │
// region_poseidon │ region_ecdsa │ rc_value │ filter_looking_rc │
// ┴────────────────────────┴───────────────────┴────────────────┴─────────────────┴──────────────┴──────────┴───────────────────┘
pub(crate) const COL_MEM_TX_IDX: usize = 0;
pub(crate) const COL_MEM_ENV_IDX: usize = COL_MEM_TX_IDX + 1;
pub(crate) const COL_MEM_IS_RW: usize = COL_MEM_ENV_IDX + 1;
pub(crate) const COL_MEM_ADDR: usize = COL_MEM_IS_RW + 1;
pub(crate) const COL_MEM_CLK: usize = COL_MEM_ADDR + 1;
pub(crate) const COL_MEM_OP: usize = COL_MEM_CLK + 1;
pub(crate) const COL_MEM_S_MLOAD: usize = COL_MEM_OP + 1;
pub(crate) const COL_MEM_S_MSTORE: usize = COL_MEM_S_MLOAD + 1;
pub(crate) const COL_MEM_S_CALL: usize = COL_MEM_S_MSTORE + 1;
pub(crate) const COL_MEM_S_RET: usize = COL_MEM_S_CALL + 1;
pub(crate) const COL_MEM_S_TLOAD: usize = COL_MEM_S_RET + 1;
pub(crate) const COL_MEM_S_TSTORE: usize = COL_MEM_S_TLOAD + 1;
pub(crate) const COL_MEM_S_SCCALL: usize = COL_MEM_S_TSTORE + 1;
pub(crate) const COL_MEM_S_POSEIDON: usize = COL_MEM_S_SCCALL + 1;
pub(crate) const COL_MEM_S_SSTORE: usize = COL_MEM_S_POSEIDON + 1;
pub(crate) const COL_MEM_S_SLOAD: usize = COL_MEM_S_SSTORE + 1;
pub(crate) const COL_MEM_S_PROPHET: usize = COL_MEM_S_SLOAD + 1;
pub(crate) const COL_MEM_IS_WRITE: usize = COL_MEM_S_PROPHET + 1;
pub(crate) const COL_MEM_VALUE: usize = COL_MEM_IS_WRITE + 1;
pub(crate) const COL_MEM_DIFF_ADDR: usize = COL_MEM_VALUE + 1;
pub(crate) const COL_MEM_DIFF_ADDR_INV: usize = COL_MEM_DIFF_ADDR + 1;
pub(crate) const COL_MEM_DIFF_CLK: usize = COL_MEM_DIFF_ADDR_INV + 1;
pub(crate) const COL_MEM_DIFF_ADDR_COND: usize = COL_MEM_DIFF_CLK + 1;
// pub(crate) const COL_MEM_FILTER_LOOKED_FOR_MAIN: usize =
// COL_MEM_DIFF_ADDR_COND + 1; pub(crate) const
// COL_MEM_FILTER_LOOKED_FOR_POSEIDON_CHUNK: usize =
// COL_MEM_FILTER_LOOKED_FOR_MAIN + 1;
pub(crate) const COL_MEM_RW_ADDR_UNCHANGED: usize = COL_MEM_DIFF_ADDR_COND + 1;
pub(crate) const COL_MEM_REGION_PROPHET: usize = COL_MEM_RW_ADDR_UNCHANGED + 1;
pub(crate) const COL_MEM_REGION_HEAP: usize = COL_MEM_REGION_PROPHET + 1;
pub(crate) const COL_MEM_RC_VALUE: usize = COL_MEM_REGION_HEAP + 1;
pub(crate) const COL_MEM_FILTER_LOOKING_RC: usize = COL_MEM_RC_VALUE + 1;
pub(crate) const COL_MEM_FILTER_LOOKING_RC_COND: usize = COL_MEM_FILTER_LOOKING_RC + 1;
pub(crate) const NUM_MEM_COLS: usize = COL_MEM_FILTER_LOOKING_RC_COND + 1;

pub(crate) fn get_memory_col_name_map() -> BTreeMap<usize, String> {
    let mut m: BTreeMap<usize, String> = BTreeMap::new();
    m.insert(COL_MEM_TX_IDX, String::from("TX_IDX"));
    m.insert(COL_MEM_ENV_IDX, String::from("ENV_IDX"));
    m.insert(COL_MEM_IS_RW, String::from("IS_RW"));
    m.insert(COL_MEM_ADDR, String::from("ADDR"));
    m.insert(COL_MEM_CLK, String::from("CLK"));
    m.insert(COL_MEM_OP, String::from("OP"));
    m.insert(COL_MEM_S_MLOAD, String::from("S_MLOAD"));
    m.insert(COL_MEM_S_MSTORE, String::from("S_MSTORE"));
    m.insert(COL_MEM_S_CALL, String::from("S_CALL"));
    m.insert(COL_MEM_S_RET, String::from("S_RET"));
    m.insert(COL_MEM_S_TLOAD, String::from("S_TLOAD"));
    m.insert(COL_MEM_S_TSTORE, String::from("S_TSTORE"));
    m.insert(COL_MEM_S_SCCALL, String::from("S_SCCALL"));
    m.insert(COL_MEM_S_POSEIDON, String::from("S_POSEIDON"));
    m.insert(COL_MEM_S_SSTORE, String::from("S_SSTORE"));
    m.insert(COL_MEM_S_SLOAD, String::from("S_SLOAD"));
    m.insert(COL_MEM_S_PROPHET, String::from("S_PROPHET"));
    m.insert(COL_MEM_IS_WRITE, String::from("IS_WRITE"));
    m.insert(COL_MEM_VALUE, String::from("VALUE"));
    m.insert(COL_MEM_DIFF_ADDR, String::from("DIFF_ADDR"));
    m.insert(COL_MEM_DIFF_ADDR_INV, String::from("DIFF_ADDR_INV"));
    m.insert(COL_MEM_DIFF_CLK, String::from("DIFF_CLK"));
    m.insert(COL_MEM_DIFF_ADDR_COND, String::from("DIFF_ADDR_COND"));
    m.insert(COL_MEM_RW_ADDR_UNCHANGED, String::from("RW_ADDR_UNCHANGED"));
    m.insert(COL_MEM_REGION_PROPHET, String::from("REGION_PROPHET"));
    m.insert(COL_MEM_REGION_HEAP, String::from("REGION_HEAP"));
    m.insert(COL_MEM_RC_VALUE, String::from("RC_VALUE"));
    m.insert(COL_MEM_FILTER_LOOKING_RC, String::from("FILTER_LOOKING_RC"));
    m.insert(
        COL_MEM_FILTER_LOOKING_RC_COND,
        String::from("FILTER_LOOKING_RC_COND"),
    );
    m
}

#[test]
fn print_memory_cols() {
    let m = get_memory_col_name_map();
    for (col, name) in m {
        println!("{}: {}", col, name);
    }
}
