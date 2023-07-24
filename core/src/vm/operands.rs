use regex::Regex;
use std::fmt::{Display, Formatter};
use std::i128;
use std::num::ParseIntError;
use std::str::FromStr;

use super::hardware::{OlaRegister, OlaSpecialRegister};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum OlaOperand {
    ImmediateOperand {
        value: ImmediateValue,
    },
    RegisterOperand {
        register: OlaRegister,
    },
    RegisterWithOffset {
        register: OlaRegister,
        offset: ImmediateValue,
    },
    RegisterWithFactor {
        register: OlaRegister,
        factor: ImmediateValue,
    },
    SpecialReg {
        special_reg: OlaSpecialRegister,
    },
}

impl OlaOperand {
    pub fn get_asm_token(&self) -> String {
        match self {
            OlaOperand::ImmediateOperand { value } => value.clone().hex,
            OlaOperand::RegisterOperand { register } => {
                format!("{}", register)
            }
            OlaOperand::RegisterWithOffset { register, offset } => {
                format!("[{},{}]", register, offset.hex)
            }
            OlaOperand::SpecialReg { special_reg } => {
                format!("{}", special_reg)
            }
            OlaOperand::RegisterWithFactor { register, factor } => {
                format!("{}*{}", factor.hex, register)
            }
        }
    }
}

impl FromStr for OlaOperand {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let regex_reg_offset =
            Regex::new(r"^\[(?P<reg>r[0-8]),(?P<offset>-?[[:digit:]]+)\]$").unwrap();
        let capture_reg_offset = regex_reg_offset.captures(s);
        if capture_reg_offset.is_some() {
            let caps = capture_reg_offset.unwrap();
            let str_reg = caps.name("reg").unwrap().as_str();
            let str_offset = caps.name("offset").unwrap().as_str();
            let register = OlaRegister::from_str(str_reg)?;
            let offset = ImmediateValue::from_str(str_offset)?;
            return Ok(OlaOperand::RegisterWithOffset { register, offset });
        }

        let regex_reg = Regex::new(r"^(?P<reg>r[0-8])$").unwrap();
        let capture_reg = regex_reg.captures(s);
        if capture_reg.is_some() {
            let caps = capture_reg.unwrap();
            let str_reg = caps.name("reg").unwrap().as_str();
            let register = OlaRegister::from_str(str_reg)?;
            return Ok(OlaOperand::RegisterOperand { register });
        }

        let regex_immediate_value = Regex::new(r"^(?P<imm>-?[[:digit:]]+)$").unwrap();
        let capture_immediate = regex_immediate_value.captures(s);
        if capture_immediate.is_some() {
            let caps = capture_immediate.unwrap();
            let str_imm = caps.name("imm").unwrap().as_str();
            let value = ImmediateValue::from_str(str_imm)?;
            return Ok(OlaOperand::ImmediateOperand { value });
        }

        let special_reg = OlaSpecialRegister::from_str(s);
        if special_reg.is_ok() {
            return Ok(OlaOperand::SpecialReg {
                special_reg: special_reg.unwrap(),
            });
        }

        return Err(format!("invalid operand: {}", s));
    }
}

impl Display for OlaOperand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            OlaOperand::ImmediateOperand { value } => {
                write!(f, "ImmediateOperand({})", value)
            }
            OlaOperand::RegisterOperand { register } => {
                write!(f, "RegisterOperand({})", register)
            }
            OlaOperand::RegisterWithOffset { register, offset } => {
                write!(
                    f,
                    "RegisterWithOffset([{},{}])",
                    register,
                    offset.to_u64().unwrap_or(0)
                )
            }
            OlaOperand::SpecialReg { special_reg } => {
                write!(f, "SpecialReg({})", special_reg)
            }
            OlaOperand::RegisterWithFactor { register, factor } => {
                write!(
                    f,
                    "RegisterWithFactor({}*{})",
                    factor.to_u64().unwrap_or(0),
                    register
                )
            }
        }
    }
}

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct ImmediateValue {
    pub hex: String,
}

impl ImmediateValue {
    const ORDER: u64 = 0xFFFFFFFF00000001;
    pub fn to_u64(&self) -> Result<u64, ParseIntError> {
        let without_prefix = self.hex.trim_start_matches("0x");
        return u64::from_str_radix(without_prefix, 16);
    }
}

impl Display for ImmediateValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let hex = self.hex.clone();
        let value = self.to_u64().unwrap_or(0);
        write!(f, "{}({})", hex, value)
    }
}

impl FromStr for ImmediateValue {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("0x") {
            let without_prefix = s.trim_start_matches("0x");
            let hex_parsed_res = u64::from_str_radix(without_prefix, 16);
            if hex_parsed_res.is_err() {
                return Err(format!("Immediate is not a valid number: {}", s));
            }
            let value = hex_parsed_res.unwrap();
            if value >= ImmediateValue::ORDER {
                return Err(format!("Immediate overflow: {}", s));
            }
            return Ok(ImmediateValue {
                hex: format!("{:#x}", value),
            });
        }

        let parsed_result = i128::from_str_radix(s, 10);
        if parsed_result.is_err() {
            return Err(format!("Immediate is not a valid number: {}", s));
        }
        let value = parsed_result.unwrap();
        let signed_order = ImmediateValue::ORDER as i128;
        if value >= signed_order || value * -1 >= signed_order {
            return Err(format!("Immediate overflow: {}", s));
        }
        let actual_value = if value < 0 {
            signed_order - value.abs()
        } else {
            value
        } as u64;
        Ok(ImmediateValue {
            hex: format!("{:#x}", actual_value),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::vm::operands::{ImmediateValue, OlaOperand, OlaRegister, OlaSpecialRegister};
    use std::str::FromStr;

    #[test]
    fn test_immediate_parse() {
        let overflow_upper = ImmediateValue::from_str("0xffffffff00000002");
        let err_str = "wtf".to_string();
        assert!(matches!(overflow_upper, Err(err_str)));
        let immediate_999 = ImmediateValue::from_str("999").unwrap();
        assert_eq!(
            immediate_999,
            ImmediateValue {
                hex: "0x3e7".to_string()
            }
        );

        let value_u64 = immediate_999.to_u64().unwrap();
        assert_eq!(value_u64, 999);

        let hex_value = ImmediateValue::from_str("0xffffffff00000000").unwrap();
        assert_eq!(
            hex_value,
            ImmediateValue {
                hex: String::from("0xffffffff00000000")
            }
        );
    }

    #[test]
    fn test_operand_parse() {
        let oper_reg = OlaOperand::from_str("r6").unwrap();
        assert_eq!(
            oper_reg,
            OlaOperand::RegisterOperand {
                register: OlaRegister::R6
            }
        );

        let oper_reg_offset = OlaOperand::from_str("[r0,-7]").unwrap();
        assert_eq!(
            oper_reg_offset,
            OlaOperand::RegisterWithOffset {
                register: OlaRegister::R0,
                offset: ImmediateValue::from_str("-7").unwrap()
            }
        );

        let oper_imm = OlaOperand::from_str("-999").unwrap();
        assert_eq!(
            oper_imm,
            OlaOperand::ImmediateOperand {
                value: ImmediateValue::from_str("-999").unwrap()
            }
        );

        let oper_psp = OlaOperand::from_str("psp").unwrap();
        assert_eq!(
            oper_psp,
            OlaOperand::SpecialReg {
                special_reg: OlaSpecialRegister::PSP
            }
        )
    }
}
