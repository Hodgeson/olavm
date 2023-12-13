use crate::asm::OlaAsmInstruction;
use crate::operands::OlaAsmOperand;
use crate::relocate::{asm_relocate, AsmBundle, RelocatedAsmBundle};
use core::program::binary_program::{BinaryInstruction, BinaryProgram, OlaProphet};
use core::vm::opcodes::OlaOpcode;
use core::vm::operands::{ImmediateValue, OlaOperand};
use log::debug;
use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;

pub fn encode_asm_from_json_file(path: String) -> Result<BinaryProgram, String> {
    let json_str = std::fs::read_to_string(path).unwrap();
    let bundle: AsmBundle = serde_json::from_str(json_str.as_str()).unwrap();
    let relocated = asm_relocate(bundle).unwrap();
    let program = encode_to_binary(relocated).unwrap();
    Ok(program)
}

pub(crate) fn encode_to_binary(bundle: RelocatedAsmBundle) -> Result<BinaryProgram, String> {
    let asm_instructions = bundle.instructions;
    let mapper_label_call = &bundle.mapper_label_call.clone();
    let mapper_label_jmp = &bundle.mapper_label_jmp.clone();
    let asm_prophets = &bundle.prophets;

    let mut binary_instructions: Vec<BinaryInstruction> = vec![];
    let mut iter = asm_instructions.iter();
    let mut binary_counter: usize = 0;
    let mut origin_asm = BTreeMap::new();

    while let Some(asm) = iter.next() {
        let ops_result: Result<
            (Option<OlaOperand>, Option<OlaOperand>, Option<OlaOperand>),
            String,
        > = if is_adjusted_operand(asm) {
            let r = handle_mem_operand(asm);
            if r.is_err() {
                return Err(format!(
                    "relocated asm to binary error: ops convert error ==> {}",
                    r.err().unwrap()
                ));
            } else {
                let tuple = r.unwrap();
                Ok((Some(tuple.0), Some(tuple.1), Some(tuple.2)))
            }
        } else {
            let op0_result =
                operand_asm_to_binary(asm.clone().op0, mapper_label_call, mapper_label_jmp);
            if op0_result.is_err() {
                return Err(format!(
                    "relocated asm to binary error: op0 convert error ==> {}",
                    op0_result.err().unwrap()
                ));
            }
            let op0 = op0_result.unwrap();

            let op1_result =
                operand_asm_to_binary(asm.clone().op1, mapper_label_call, mapper_label_jmp);
            if op1_result.is_err() {
                return Err(format!(
                    "relocated asm to binary error: op1 convert error ==> {}",
                    op1_result.err().unwrap()
                ));
            }
            let op1 = op1_result.unwrap();

            let dst_result =
                operand_asm_to_binary(asm.clone().dst, mapper_label_call, mapper_label_jmp);
            if dst_result.is_err() {
                return Err(format!(
                    "relocated asm to binary error: dst convert error ==> {}",
                    dst_result.err().unwrap()
                ));
            }
            let dst = dst_result.unwrap();
            Ok((op0, op1, dst))
        };
        if ops_result.is_err() {
            return Err(format!(
                "relocated asm to binary error: ==> {}",
                ops_result.err().unwrap()
            ));
        }
        let (op0, op1, dst) = ops_result.unwrap();

        let prophet: Option<OlaProphet> =
            if let Some(asm_prophet) = asm_prophets.get(&binary_counter) {
                Some(OlaProphet {
                    host: binary_counter.clone(),
                    code: asm_prophet.code.clone(),
                    ctx: Vec::new(),
                    inputs: asm_prophet.inputs.clone(),
                    outputs: asm_prophet.outputs.clone(),
                })
            } else {
                None
            };

        let instruction = BinaryInstruction {
            opcode: asm.opcode,
            op0,
            op1,
            dst,
            prophet,
        };
        origin_asm.insert(binary_counter, asm.asm.clone());
        debug!(
            "binary_counter:{}, asm:{}, code:{}",
            binary_counter, asm.asm, instruction
        );
        binary_instructions.push(instruction);
        binary_counter += asm.binary_length() as usize;
    }
    BinaryProgram::from_instructions(binary_instructions, Some(origin_asm), true)
}

fn is_adjusted_operand(asm: &OlaAsmInstruction) -> bool {
    if asm.opcode == OlaOpcode::MLOAD || asm.opcode == OlaOpcode::MSTORE {
        true
    } else {
        false
    }
}

fn handle_mem_operand(
    asm: &OlaAsmInstruction,
) -> Result<(OlaOperand, OlaOperand, OlaOperand), String> {
    let dst = if asm.opcode == OlaOpcode::MLOAD {
        asm.dst.clone()
    } else {
        asm.op1.clone()
    }
    .unwrap();
    let dst_reg = match dst {
        OlaAsmOperand::RegisterOperand { register } => OlaOperand::RegisterOperand { register },
        _ => {
            panic!("parse dst reg error")
        }
    };

    let anchor_reg = if asm.opcode == OlaOpcode::MLOAD {
        match asm.op1.clone().unwrap() {
            OlaAsmOperand::RegisterWithOffset {
                register,
                offset: _,
            } => OlaOperand::RegisterOperand { register },
            OlaAsmOperand::RegisterWithFactoredRegOffset {
                register,
                offset_register: _,
                factor: _,
            } => OlaOperand::RegisterOperand { register },
            _ => {
                panic!("parse anchor reg error")
            }
        }
    } else {
        match asm.op0.clone().unwrap() {
            OlaAsmOperand::RegisterWithOffset {
                register,
                offset: _,
            } => OlaOperand::RegisterOperand { register },
            OlaAsmOperand::RegisterWithFactoredRegOffset {
                register,
                offset_register: _,
                factor: _,
            } => OlaOperand::RegisterOperand { register },
            _ => {
                panic!("parse anchor reg error")
            }
        }
    };

    let offset = if asm.opcode == OlaOpcode::MLOAD {
        match asm.op1.clone().unwrap() {
            OlaAsmOperand::RegisterWithOffset {
                register: _,
                offset,
            } => OlaOperand::ImmediateOperand { value: offset },
            OlaAsmOperand::RegisterWithFactoredRegOffset {
                register: _,
                offset_register,
                factor,
            } => OlaOperand::RegisterWithFactor {
                register: offset_register,
                factor,
            },
            _ => {
                panic!("parse offset error")
            }
        }
    } else {
        match asm.op0.clone().unwrap() {
            OlaAsmOperand::RegisterWithOffset {
                register: _,
                offset,
            } => OlaOperand::ImmediateOperand { value: offset },
            OlaAsmOperand::RegisterWithFactoredRegOffset {
                register: _,
                offset_register,
                factor,
            } => OlaOperand::RegisterWithFactor {
                register: offset_register,
                factor,
            },
            _ => {
                panic!("parse offset error")
            }
        }
    };

    Ok((anchor_reg, offset, dst_reg))
}

fn operand_asm_to_binary(
    option_asm_op: Option<OlaAsmOperand>,
    mapper_label_call: &HashMap<String, usize>,
    mapper_label_jmp: &HashMap<String, usize>,
) -> Result<Option<OlaOperand>, String> {
    let op: Option<OlaOperand> = if let Some(asm_op) = option_asm_op {
        match asm_op {
            OlaAsmOperand::ImmediateOperand { value } => {
                Some(OlaOperand::ImmediateOperand { value })
            }
            OlaAsmOperand::RegisterOperand { register } => {
                Some(OlaOperand::RegisterOperand { register })
            }
            OlaAsmOperand::SpecialReg { .. } => None,
            OlaAsmOperand::Label { value } => {
                if let Some(host) = mapper_label_jmp.get(value.as_str()) {
                    Some(OlaOperand::ImmediateOperand {
                        value: ImmediateValue::from_str(host.to_string().as_str()).unwrap(),
                    })
                } else {
                    return Err(format!(
                        "relocated asm to binary error: invalid label {}",
                        value
                    ));
                }
            }
            OlaAsmOperand::Identifier { value } => {
                if let Some(host) = mapper_label_call.get(value.as_str()) {
                    Some(OlaOperand::ImmediateOperand {
                        value: ImmediateValue::from_str(host.to_string().as_str()).unwrap(),
                    })
                } else {
                    return Err(format!(
                        "relocated asm to binary error: invalid identifier {}",
                        value
                    ));
                }
            }
            OlaAsmOperand::RegisterWithOffset { .. } => None,
            OlaAsmOperand::RegisterWithFactoredRegOffset { .. } => None,
        }
    } else {
        None
    };
    Ok(op)
}

// #[cfg(test)]
// mod tests {
//     use crate::encoder::encode_to_binary;
//     use crate::relocate::{asm_relocate, AsmBundle};

//     #[test]
//     fn test_encode() {
//         let json = "{\"program\":\"main:\\n.LBL0_0:\\nadd r8 r8 2\\nmov r0
// 20\\nmov r1 5\\nadd r0 r0 r1\\nmov r7 r8\\nmov r8 psp\\n.PROPHET0_0:\\nmload
// r1 [r8,1]\\nmov r8 r7\\nmul r2 r1 r1\\nassert r0 r2\\nmstore [r8,-2]
// r0\\nmstore [r8,-1]
// r1\\nend\",\"prophets\":[{\"label\":\".PROPHET0_0\",\"code\":\"%{\\n  entry()
// {\\n    uint cid.y = sqrt(cid.x);\\n
// }\\n%}\",\"inputs\":[\"cid.x\"],\"outputs\":[\"cid.y\"]}]}";         let
// bundle: AsmBundle = serde_json::from_str(json).unwrap();         let
// relocated = asm_relocate(bundle).unwrap();         let program =
// encode_to_binary(relocated).unwrap();         dbg!(program);
//     }
// }
