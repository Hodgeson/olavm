use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
};

use crate::{
    error::OlaRunnerError,
    vm::ola_vm::{OlaContext, NUM_GENERAL_PURPOSE_REGISTER},
};
use anyhow::{anyhow, bail, Ok, Result};
use assembler::{
    binary_program::Prophet,
    hardware::{OlaRegister, OlaSpecialRegister},
};
use assembler::{
    binary_program::{BinaryInstruction, BinaryProgram},
    decoder::decode_binary_program_from_file,
    opcodes::OlaOpcode,
    operands::OlaOperand,
};
use interpreter::interpreter::Interpreter;
use plonky2::field::{
    goldilocks_field::GoldilocksField,
    types::{Field, PrimeField64},
};
use regex::Regex;

#[derive(Debug, Clone)]
struct IntermediateRowCpu {
    clk: u64,
    pc: u64,
    psp: u64,
    registers: [GoldilocksField; NUM_GENERAL_PURPOSE_REGISTER],
    instruction: BinaryInstruction,
    op0: GoldilocksField,
    op1: GoldilocksField,
    dst: GoldilocksField,
    aux0: GoldilocksField,
    aux1: GoldilocksField,
}

#[derive(Debug, Clone)]
struct IntermediateRowMemory {
    addr: u64,
    value: GoldilocksField,
    is_write: bool,
    opcode: Option<OlaOpcode>,
}

#[derive(Debug, Clone)]
enum RangeCheckRequester {
    Cpu,
    Memory,
    Comparison,
}
#[derive(Debug, Clone)]
struct IntermediateRowRangeCheck {
    value: GoldilocksField,
    requester: RangeCheckRequester,
}

#[derive(Debug, Clone)]
struct IntermediateRowBitwise {
    opcode: GoldilocksField,
    op0: GoldilocksField,
    op1: GoldilocksField,
    res: GoldilocksField,
}

#[derive(Debug, Clone)]
struct IntermediateRowComparison {
    op0: GoldilocksField,
    op1: GoldilocksField,
    is_gte: bool,
}

#[derive(Debug, Clone)]
struct IntermediateTraceStepAppender {
    cpu: IntermediateRowCpu,
    memory: Option<Vec<IntermediateRowMemory>>,
    range_check: Option<Vec<IntermediateRowRangeCheck>>,
    bitwise: Option<IntermediateRowBitwise>,
    comparison: Option<IntermediateRowComparison>,
}

#[derive(Debug, Clone)]
struct IntermediateTraceCollector {
    cpu: Vec<IntermediateRowCpu>,
    memory: BTreeMap<u64, Vec<IntermediateRowMemory>>,
    range_check: Vec<IntermediateRowRangeCheck>,
    bitwise: Vec<IntermediateRowBitwise>,
    comparison: Vec<IntermediateRowComparison>,
}

impl Default for IntermediateTraceCollector {
    fn default() -> Self {
        Self {
            cpu: Default::default(),
            memory: Default::default(),
            range_check: Default::default(),
            bitwise: Default::default(),
            comparison: Default::default(),
        }
    }
}

impl IntermediateTraceCollector {
    fn append(&mut self, appender: IntermediateTraceStepAppender) {
        self.cpu.push(appender.cpu);
        match appender.memory {
            Some(rows) => {
                rows.iter().for_each(|row| {
                    self.memory
                        .entry(row.addr)
                        .and_modify(|v| {
                            v.push(row.clone());
                        })
                        .or_insert_with(|| vec![row.clone()]);
                });
            }
            None => {}
        }
        match appender.range_check {
            Some(rows) => rows.iter().for_each(|row| {
                self.range_check.push(row.clone());
            }),
            None => {}
        }
        match appender.bitwise {
            Some(row) => self.bitwise.push(row.clone()),
            None => {}
        }
        match appender.comparison {
            Some(row) => self.comparison.push(row.clone()),
            None => {}
        }
    }
}

#[derive(Debug)]
pub struct OlaRunner {
    program: BinaryProgram,
    instructions: HashMap<u64, BinaryInstruction>,
    context: OlaContext,
    trace_collector: IntermediateTraceCollector,
    is_ended: bool,
}

impl OlaRunner {
    pub fn new_from_program_file(path: String) -> Result<Self> {
        let instruction_vec = match decode_binary_program_from_file(path) {
            std::result::Result::Ok(it) => it,
            Err(err) => bail!("{}", err),
        };
        Self::new_from_instruction_vec(instruction_vec)
    }

    fn new_from_instruction_vec(instruction_vec: Vec<BinaryInstruction>) -> Result<Self> {
        let mut instructions: HashMap<u64, BinaryInstruction> = HashMap::new();
        let mut index: u64 = 0;
        instruction_vec.iter().for_each(|instruction| {
            instructions.insert(index, instruction.clone());
            index += instruction.binary_length() as u64;
        });
        let program = match BinaryProgram::from_instructions(instruction_vec) {
            std::result::Result::Ok(it) => it,
            Err(err) => bail!("{}", err),
        };
        Ok(OlaRunner {
            program,
            instructions,
            context: OlaContext::default(),
            trace_collector: IntermediateTraceCollector::default(),
            is_ended: false,
        })
    }

    pub fn run_one_step(&mut self) -> Result<IntermediateTraceStepAppender> {
        if self.is_ended {
            return Err(anyhow!("{}", OlaRunnerError::RunAfterEndedError));
        }
        let instruction = match self.instructions.get(&self.context.pc) {
            Some(it) => it.clone(),
            None => {
                return Err(anyhow!(
                    "{}",
                    OlaRunnerError::InstructionNotFoundError {
                        clk: self.context.clk.clone(),
                        pc: self.context.pc.clone()
                    }
                ))
            }
        };

        let mut appender = match instruction.opcode {
            OlaOpcode::ADD
            | OlaOpcode::MUL
            | OlaOpcode::EQ
            | OlaOpcode::AND
            | OlaOpcode::OR
            | OlaOpcode::XOR
            | OlaOpcode::NEQ
            | OlaOpcode::GTE => self.on_two_operands_arithmetic_op(instruction.clone())?,
            OlaOpcode::ASSERT => {
                let trace_op0 = self.get_operand_value(instruction.op0.clone().unwrap())?;
                let trace_op1 = self.get_operand_value(instruction.op1.clone().unwrap())?;
                if trace_op0.0 != trace_op1.0 {
                    return Err(anyhow!(
                        "{}",
                        OlaRunnerError::AssertFailError {
                            clk: self.context.clk.clone(),
                            pc: self.context.pc.clone(),
                            op0: trace_op0.0,
                            op1: trace_op1.0
                        }
                    ));
                }
                let row_cpu = IntermediateRowCpu {
                    clk: self.context.clk.clone(),
                    pc: self.context.pc.clone(),
                    psp: self.context.psp.clone(),
                    registers: self.context.registers.clone(),
                    instruction: instruction.clone(),
                    op0: trace_op0,
                    op1: trace_op1,
                    dst: GoldilocksField::default(),
                    aux0: GoldilocksField::default(),
                    aux1: GoldilocksField::default(),
                };

                self.context.clk += 1;
                self.context.pc += instruction.binary_length() as u64;

                IntermediateTraceStepAppender {
                    cpu: row_cpu,
                    memory: None,
                    range_check: None,
                    bitwise: None,
                    comparison: None,
                }
            }

            OlaOpcode::MOV => {
                let trace_op1 = self.get_operand_value(instruction.op1.clone().unwrap())?;
                let row_cpu = IntermediateRowCpu {
                    clk: self.context.clk.clone(),
                    pc: self.context.pc.clone(),
                    psp: self.context.psp.clone(),
                    registers: self.context.registers.clone(),
                    instruction: instruction.clone(),
                    op0: GoldilocksField::default(),
                    op1: trace_op1.clone(),
                    dst: trace_op1.clone(),
                    aux0: GoldilocksField::default(),
                    aux1: GoldilocksField::default(),
                };

                self.context.clk += 1;
                self.context.pc += instruction.binary_length() as u64;
                self.update_dst_reg(trace_op1.clone(), instruction.dst.clone().unwrap())?;

                IntermediateTraceStepAppender {
                    cpu: row_cpu,
                    memory: None,
                    range_check: None,
                    bitwise: None,
                    comparison: None,
                }
            }
            OlaOpcode::JMP => {
                let trace_op1 = self.get_operand_value(instruction.op1.clone().unwrap())?;
                let row_cpu = IntermediateRowCpu {
                    clk: self.context.clk.clone(),
                    pc: self.context.pc.clone(),
                    psp: self.context.psp.clone(),
                    registers: self.context.registers.clone(),
                    instruction: instruction.clone(),
                    op0: GoldilocksField::default(),
                    op1: trace_op1.clone(),
                    dst: GoldilocksField::default(),
                    aux0: GoldilocksField::default(),
                    aux1: GoldilocksField::default(),
                };

                self.context.clk += 1;
                self.context.pc = trace_op1.clone().to_noncanonical_u64();

                IntermediateTraceStepAppender {
                    cpu: row_cpu,
                    memory: None,
                    range_check: None,
                    bitwise: None,
                    comparison: None,
                }
            }
            OlaOpcode::CJMP => {
                let trace_op0 = self.get_operand_value(instruction.op0.clone().unwrap())?;
                let trace_op1 = self.get_operand_value(instruction.op1.clone().unwrap())?;
                let flag = trace_op0.clone().to_noncanonical_u64();
                if flag != 0 && flag != 1 {
                    return Err(anyhow!(
                        "{}",
                        OlaRunnerError::FlagNotBinaryError {
                            clk: self.context.clk.clone(),
                            pc: self.context.pc.clone(),
                            opcode: instruction.opcode.token(),
                            flag: trace_op0.0
                        }
                    ));
                }
                let row_cpu = IntermediateRowCpu {
                    clk: self.context.clk.clone(),
                    pc: self.context.pc.clone(),
                    psp: self.context.psp.clone(),
                    registers: self.context.registers.clone(),
                    instruction: instruction.clone(),
                    op0: trace_op0.clone(),
                    op1: trace_op1.clone(),
                    dst: GoldilocksField::default(),
                    aux0: GoldilocksField::default(),
                    aux1: GoldilocksField::default(),
                };

                self.context.clk += 1;
                self.context.pc = if flag == 1 {
                    trace_op1.clone().to_noncanonical_u64()
                } else {
                    self.context.pc + instruction.binary_length() as u64
                };

                IntermediateTraceStepAppender {
                    cpu: row_cpu,
                    memory: None,
                    range_check: None,
                    bitwise: None,
                    comparison: None,
                }
            }
            OlaOpcode::CALL => {
                let trace_op0 = self.context.get_fp().clone() - GoldilocksField(1);
                let trace_op1 = self.get_operand_value(instruction.op1.clone().unwrap())?;
                let trace_dst =
                    GoldilocksField(self.context.pc + instruction.binary_length() as u64);
                let trace_aux0 = self.context.get_fp().clone() - GoldilocksField(2);
                let trace_aux1 = self
                    .context
                    .memory
                    .read(trace_aux0.clone().to_canonical_u64())?;

                let row_cpu = IntermediateRowCpu {
                    clk: self.context.clk.clone(),
                    pc: self.context.pc.clone(),
                    psp: self.context.psp.clone(),
                    registers: self.context.registers.clone(),
                    instruction: instruction.clone(),
                    op0: trace_op0.clone(),
                    op1: trace_op1.clone(),
                    dst: trace_dst.clone(),
                    aux0: trace_aux0.clone(),
                    aux1: trace_aux1.clone(),
                };

                let rows_memory = vec![
                    IntermediateRowMemory {
                        addr: trace_op0.clone().to_canonical_u64(),
                        value: trace_dst.clone(),
                        is_write: true,
                        opcode: Some(OlaOpcode::CALL),
                    },
                    IntermediateRowMemory {
                        addr: trace_aux0.clone().to_canonical_u64(),
                        value: trace_aux1.clone(),
                        is_write: false,
                        opcode: Some(OlaOpcode::CALL),
                    },
                ];

                self.context.clk += 1;
                self.context.pc = trace_op1.clone().to_canonical_u64();
                self.context.memory.store_in_segment_read_write(
                    trace_op0.clone().to_canonical_u64(),
                    trace_dst.clone(),
                );

                IntermediateTraceStepAppender {
                    cpu: row_cpu,
                    memory: Some(rows_memory),
                    range_check: None,
                    bitwise: None,
                    comparison: None,
                }
            }
            OlaOpcode::RET => {
                let trace_op0 = self.context.get_fp().clone() - GoldilocksField(1);
                let trace_dst = self
                    .context
                    .memory
                    .read(trace_op0.clone().to_canonical_u64())?;
                let trace_aux0 = self.context.get_fp().clone() - GoldilocksField(2);
                let trace_aux1 = self
                    .context
                    .memory
                    .read(trace_aux0.clone().to_canonical_u64())?;

                let row_cpu = IntermediateRowCpu {
                    clk: self.context.clk.clone(),
                    pc: self.context.pc.clone(),
                    psp: self.context.psp.clone(),
                    registers: self.context.registers.clone(),
                    instruction: instruction.clone(),
                    op0: trace_op0.clone(),
                    op1: GoldilocksField::default(),
                    dst: trace_dst.clone(),
                    aux0: trace_aux0.clone(),
                    aux1: trace_aux1.clone(),
                };
                let rows_memory = vec![
                    IntermediateRowMemory {
                        addr: trace_op0.clone().to_canonical_u64(),
                        value: trace_dst.clone(),
                        is_write: false,
                        opcode: Some(OlaOpcode::RET),
                    },
                    IntermediateRowMemory {
                        addr: trace_aux0.clone().to_canonical_u64(),
                        value: trace_aux1.clone(),
                        is_write: false,
                        opcode: Some(OlaOpcode::RET),
                    },
                ];

                self.context.clk += 1;
                self.context.pc = trace_dst.clone().to_canonical_u64();

                IntermediateTraceStepAppender {
                    cpu: row_cpu,
                    memory: Some(rows_memory),
                    range_check: None,
                    bitwise: None,
                    comparison: None,
                }
            }
            OlaOpcode::MLOAD => {
                let (anchor_addr, offset) =
                    self.split_register_offset_operand(instruction.op1.clone().unwrap())?;
                let addr = anchor_addr + offset;
                let trace_op1 = anchor_addr.clone();
                let trace_dst = self.context.memory.read(addr.to_canonical_u64())?;
                let trace_aux0 = offset.clone();
                let trace_aux1 = addr;

                let row_cpu = IntermediateRowCpu {
                    clk: self.context.clk.clone(),
                    pc: self.context.pc.clone(),
                    psp: self.context.psp.clone(),
                    registers: self.context.registers.clone(),
                    instruction: instruction.clone(),
                    op0: GoldilocksField::default(),
                    op1: trace_op1.clone(),
                    dst: trace_dst.clone(),
                    aux0: trace_aux0.clone(),
                    aux1: trace_aux1.clone(),
                };
                let rows_memory = vec![IntermediateRowMemory {
                    addr: addr.clone().to_canonical_u64(),
                    value: trace_dst.clone(),
                    is_write: false,
                    opcode: Some(OlaOpcode::MLOAD),
                }];

                self.context.clk += 1;
                self.context.pc += instruction.binary_length() as u64;
                self.update_dst_reg(trace_dst.clone(), instruction.op1.clone().unwrap())?;

                IntermediateTraceStepAppender {
                    cpu: row_cpu,
                    memory: Some(rows_memory),
                    range_check: None,
                    bitwise: None,
                    comparison: None,
                }
            }
            OlaOpcode::MSTORE => {
                let (anchor_addr, offset) =
                    self.split_register_offset_operand(instruction.op1.clone().unwrap())?;
                let addr = anchor_addr + offset;
                let trace_op0 = self.get_operand_value(instruction.op0.clone().unwrap())?;
                let trace_op1 = anchor_addr.clone();
                let trace_aux0 = offset.clone();
                let trace_aux1 = addr;

                let row_cpu = IntermediateRowCpu {
                    clk: self.context.clk.clone(),
                    pc: self.context.pc.clone(),
                    psp: self.context.psp.clone(),
                    registers: self.context.registers.clone(),
                    instruction: instruction.clone(),
                    op0: trace_op0.clone(),
                    op1: trace_op1.clone(),
                    dst: GoldilocksField::default(),
                    aux0: trace_aux0.clone(),
                    aux1: trace_aux1.clone(),
                };
                let rows_memory = vec![IntermediateRowMemory {
                    addr: addr.clone().to_canonical_u64(),
                    value: trace_op0.clone(),
                    is_write: true,
                    opcode: Some(OlaOpcode::MSTORE),
                }];

                self.context.clk += 1;
                self.context.pc += instruction.binary_length() as u64;

                IntermediateTraceStepAppender {
                    cpu: row_cpu,
                    memory: Some(rows_memory),
                    range_check: None,
                    bitwise: None,
                    comparison: None,
                }
            }
            OlaOpcode::END => {
                let row_cpu = IntermediateRowCpu {
                    clk: self.context.clk.clone(),
                    pc: self.context.pc.clone(),
                    psp: self.context.psp.clone(),
                    registers: self.context.registers.clone(),
                    instruction: instruction.clone(),
                    op0: GoldilocksField::default(),
                    op1: GoldilocksField::default(),
                    dst: GoldilocksField::default(),
                    aux0: GoldilocksField::default(),
                    aux1: GoldilocksField::default(),
                };

                self.is_ended = true;

                IntermediateTraceStepAppender {
                    cpu: row_cpu,
                    memory: None,
                    range_check: None,
                    bitwise: None,
                    comparison: None,
                }
            }
            OlaOpcode::RC => {
                let trace_op1 = self.get_operand_value(instruction.op1.clone().unwrap())?;
                let value = trace_op1.clone().to_canonical_u64();
                if value >= 1 << 32 {
                    return Err(anyhow!("{}", OlaRunnerError::RangeCheckFailedError(value)));
                }
                let row_cpu = IntermediateRowCpu {
                    clk: self.context.clk.clone(),
                    pc: self.context.pc.clone(),
                    psp: self.context.psp.clone(),
                    registers: self.context.registers.clone(),
                    instruction: instruction.clone(),
                    op0: GoldilocksField::default(),
                    op1: trace_op1.clone(),
                    dst: GoldilocksField::default(),
                    aux0: GoldilocksField::default(),
                    aux1: GoldilocksField::default(),
                };
                let rows_range_check = vec![IntermediateRowRangeCheck {
                    value: trace_op1.clone(),
                    requester: RangeCheckRequester::Cpu,
                }];

                self.context.clk += 1;
                self.context.pc += instruction.binary_length() as u64;

                IntermediateTraceStepAppender {
                    cpu: row_cpu,
                    memory: None,
                    range_check: Some(rows_range_check),
                    bitwise: None,
                    comparison: None,
                }
            }
            OlaOpcode::NOT => {
                let trace_op1 = self.get_operand_value(instruction.op1.clone().unwrap())?;
                let trace_dst = GoldilocksField::NEG_ONE - trace_op1;
                let row_cpu = IntermediateRowCpu {
                    clk: self.context.clk.clone(),
                    pc: self.context.pc.clone(),
                    psp: self.context.psp.clone(),
                    registers: self.context.registers.clone(),
                    instruction: instruction.clone(),
                    op0: GoldilocksField::default(),
                    op1: trace_op1.clone(),
                    dst: trace_dst.clone(),
                    aux0: GoldilocksField::default(),
                    aux1: GoldilocksField::default(),
                };

                self.context.clk += 1;
                self.context.pc += instruction.binary_length() as u64;
                self.update_dst_reg(trace_dst.clone(), instruction.dst.clone().unwrap())?;

                IntermediateTraceStepAppender {
                    cpu: row_cpu,
                    memory: None,
                    range_check: None,
                    bitwise: None,
                    comparison: None,
                }
            }
        };

        match &instruction.prophet {
            Some(prophet) => {
                let rows_memory_prophet = self.on_prophet(prophet)?;
                match appender.memory {
                    Some(memory) => {
                        let mut appended = memory.clone();
                        rows_memory_prophet.iter().for_each(|row| {
                            appended.push(row.clone());
                        });
                        appender = IntermediateTraceStepAppender {
                            cpu: appender.cpu.clone(),
                            memory: Some(appended),
                            range_check: appender.range_check.clone(),
                            bitwise: appender.bitwise.clone(),
                            comparison: appender.comparison.clone(),
                        }
                    }
                    None => {
                        appender = IntermediateTraceStepAppender {
                            cpu: appender.cpu.clone(),
                            memory: Some(rows_memory_prophet),
                            range_check: appender.range_check.clone(),
                            bitwise: appender.bitwise.clone(),
                            comparison: appender.comparison.clone(),
                        }
                    }
                }
            }
            None => {}
        }

        Ok(appender)
    }

    fn on_two_operands_arithmetic_op(
        &mut self,
        instruction: BinaryInstruction,
    ) -> Result<IntermediateTraceStepAppender> {
        let mut row_bitwise: Option<IntermediateRowBitwise> = None;
        let mut row_comparison: Option<IntermediateRowComparison> = None;
        let mut aux0 = GoldilocksField::default();

        let trace_op0 = self.get_operand_value(instruction.op0.clone().unwrap())?;
        let trace_op1 = self.get_operand_value(instruction.op1.clone().unwrap())?;
        let trace_dst: GoldilocksField = match instruction.opcode {
            OlaOpcode::ADD => trace_op0 + trace_op1,
            OlaOpcode::MUL => trace_op0 * trace_op1,
            OlaOpcode::EQ => {
                let eq = trace_op0.0 == trace_op1.0;
                aux0 = if eq {
                    GoldilocksField::default()
                } else {
                    (trace_op0 - trace_op1).inverse()
                };
                GoldilocksField(eq as u64)
            }
            OlaOpcode::AND => {
                let result = trace_op0.0 & trace_op1.0;
                row_bitwise = Some(IntermediateRowBitwise {
                    opcode: GoldilocksField(instruction.opcode.binary_bit_mask()),
                    op0: trace_op0.clone(),
                    op1: trace_op1.clone(),
                    res: GoldilocksField(result),
                });
                GoldilocksField(result)
            }
            OlaOpcode::OR => {
                let result = trace_op0.0 | trace_op1.0;
                row_bitwise = Some(IntermediateRowBitwise {
                    opcode: GoldilocksField(instruction.opcode.binary_bit_mask()),
                    op0: trace_op0.clone(),
                    op1: trace_op1.clone(),
                    res: GoldilocksField(result),
                });
                GoldilocksField(result)
            }
            OlaOpcode::XOR => {
                let result = trace_op0.0 ^ trace_op1.0;
                row_bitwise = Some(IntermediateRowBitwise {
                    opcode: GoldilocksField(instruction.opcode.binary_bit_mask()),
                    op0: trace_op0.clone(),
                    op1: trace_op1.clone(),
                    res: GoldilocksField(result),
                });
                GoldilocksField(result)
            }
            OlaOpcode::NEQ => {
                let neq = trace_op0.0 != trace_op1.0;
                aux0 = if neq {
                    (trace_op0 - trace_op1).inverse()
                } else {
                    GoldilocksField::default()
                };
                GoldilocksField(neq as u64)
            }
            OlaOpcode::GTE => {
                row_comparison = Some(IntermediateRowComparison {
                    op0: trace_op0.clone(),
                    op1: trace_op1.clone(),
                    is_gte: true,
                });
                GoldilocksField((trace_op0.0 >= trace_op1.0) as u64)
            }
            _ => bail!(
                "invalid two operands arithmetic opcode {}",
                instruction.opcode.clone()
            ),
        };
        let row_cpu = IntermediateRowCpu {
            clk: self.context.clk.clone(),
            pc: self.context.pc.clone(),
            psp: self.context.psp.clone(),
            registers: self.context.registers.clone(),
            instruction: instruction.clone(),
            op0: trace_op0,
            op1: trace_op1,
            dst: trace_dst.clone(),
            aux0: aux0.clone(),
            aux1: GoldilocksField::default(),
        };

        self.context.clk += 1;
        self.context.pc += instruction.binary_length() as u64;
        self.update_dst_reg(trace_dst.clone(), instruction.dst.clone().unwrap())?;

        Ok(IntermediateTraceStepAppender {
            cpu: row_cpu,
            memory: None,
            range_check: None,
            bitwise: row_bitwise,
            comparison: row_comparison,
        })
    }

    fn get_operand_value(&self, operand: OlaOperand) -> Result<GoldilocksField> {
        match operand {
            OlaOperand::ImmediateOperand { value } => Ok(GoldilocksField(value.to_u64()?)),
            OlaOperand::RegisterOperand { register } => Ok(self.get_register_value(register)),
            OlaOperand::RegisterWithOffset { register, offset } => {
                Ok(self.get_register_value(register) + GoldilocksField(offset.to_u64()?))
            }
            OlaOperand::SpecialReg { special_reg } => match special_reg {
                OlaSpecialRegister::PC => {
                    bail!("pc cannot be an operand {}", 1)
                }
                OlaSpecialRegister::PSP => Ok(GoldilocksField(self.context.psp.clone())),
            },
        }
    }

    fn get_register_value(&self, register: OlaRegister) -> GoldilocksField {
        match register {
            OlaRegister::R0 => self.context.registers[0].clone(),
            OlaRegister::R1 => self.context.registers[1].clone(),
            OlaRegister::R2 => self.context.registers[2].clone(),
            OlaRegister::R3 => self.context.registers[3].clone(),
            OlaRegister::R4 => self.context.registers[4].clone(),
            OlaRegister::R5 => self.context.registers[5].clone(),
            OlaRegister::R6 => self.context.registers[6].clone(),
            OlaRegister::R7 => self.context.registers[7].clone(),
            OlaRegister::R8 => self.context.registers[8].clone(),
        }
    }

    fn split_register_offset_operand(
        &self,
        operand: OlaOperand,
    ) -> Result<(GoldilocksField, GoldilocksField)> {
        match operand {
            OlaOperand::RegisterWithOffset { register, offset } => Ok((
                self.get_register_value(register),
                GoldilocksField(offset.to_u64()?),
            )),
            _ => bail!("error split anchor and offset"),
        }
    }

    fn update_dst_reg(&mut self, result: GoldilocksField, dst_operand: OlaOperand) -> Result<()> {
        match dst_operand {
            OlaOperand::ImmediateOperand { value } => bail!("invalid dst operand {}", value),
            OlaOperand::RegisterOperand { register } => match register {
                OlaRegister::R0 => self.context.registers[0] = result,
                OlaRegister::R1 => self.context.registers[1] = result,
                OlaRegister::R2 => self.context.registers[2] = result,
                OlaRegister::R3 => self.context.registers[3] = result,
                OlaRegister::R4 => self.context.registers[4] = result,
                OlaRegister::R5 => self.context.registers[5] = result,
                OlaRegister::R6 => self.context.registers[6] = result,
                OlaRegister::R7 => self.context.registers[7] = result,
                OlaRegister::R8 => self.context.registers[8] = result,
            },
            OlaOperand::RegisterWithOffset { register, offset } => {
                bail!("invalid dst operand {}-{}", register, offset)
            }
            OlaOperand::SpecialReg { special_reg } => bail!("invalid dst operand {}", special_reg),
        }
        Ok(())
    }

    fn on_prophet(&mut self, prophet: &Prophet) -> Result<Vec<IntermediateRowMemory>> {
        let mut rows_memory: Vec<IntermediateRowMemory> = vec![];

        let re = Regex::new(r"^%\{([\s\S]*)%}$").unwrap();
        let code = re.captures(&prophet.code).unwrap().get(1).unwrap().as_str();
        let mut interpreter = Interpreter::new(code);
        let mut values = Vec::new();
        for input in prophet.inputs.iter() {
            if input.stored_in.eq("reg") {
                let register_res = OlaRegister::from_str(&input.anchor);
                match register_res {
                    std::result::Result::Ok(register) => {
                        values.push(self.get_register_value(register).to_canonical_u64())
                    }
                    Err(err) => return Err(anyhow!("{}", err)),
                }
            }
        }
        let prophet_result = interpreter.run(prophet, values);
        match prophet_result {
            std::result::Result::Ok(result) => match result {
                interpreter::utils::number::NumberRet::Single(_) => {
                    return Err(anyhow!("{}", OlaRunnerError::ProphetReturnTypeError))
                }
                interpreter::utils::number::NumberRet::Multiple(values) => {
                    for value in values {
                        rows_memory.push(IntermediateRowMemory {
                            addr: self.context.psp.clone(),
                            value: GoldilocksField(value.get_number() as u64),
                            is_write: true,
                            opcode: None,
                        })
                    }
                    self.context.psp += 1;
                }
            },
            Err(err) => return Err(anyhow!("{}", err)),
        }

        Ok(rows_memory)
    }
}
