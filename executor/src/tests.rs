use crate::Process;
use core::program::Program;
use log::debug;
use std::fs::File;
use std::io::Write;
use std::time::Instant;

#[test]
fn add_mul_decode() {
    //main:
    //  .LBL_0_0:
    //    add r8 r8 4
    //    mov r4 100
    //    not r5 3
    //    add r5 r5 1
    //    add r5 r8 r5
    //    mstore r5 r4
    //    mov r4 1
    //    not r6 2
    //    add r6 r6 1
    //    add r6 r8 r6
    //    mstore r6 r4
    //    mov r4 2
    //    not r7 1
    //    add r7 r7 1
    //    add r7 r8 r7
    //    mstore r7 r4
    //    mload r4 r6
    //    mload r1 r7
    //    mload r0 r5
    //    add r4 r4 r1
    //    mul r4 r4 r0
    //    mstore r5 r4
    //    mload r0 r5
    //    not r4 4
    //    add r4 r4 1
    //    add r8 r8 r4
    //    end
    let program_src = "0x6000080400000000
                             0x4
                             0x4000008040000000
                             0x64
                             0x4000010000040000
                             0x3
                             0x4400010400000000
                             0x1
                             0x2002010400000000
                             0x0202000001000000
                             0x4000008040000000
                             0x1
                             0x4000020000040000
                             0x2
                             0x4800020400000000
                             0x1
                             0x2004020400000000
                             0x0204000001000000
                             0x4000008040000000
                             0x2
                             0x4000040000040000
                             0x1
                             0x5000040400000000
                             0x1
                             0x2008040400000000
                             0x0208000001000000
                             0x0004008002000000
                             0x0008001002000000
                             0x0002000802000000
                             0x0200208400000000
                             0x0200108200000000
                             0x0202000001000000
                             0x0002000802000000
                             0x4000008000040000
                             0x4
                             0x4200008400000000
                             0x1
                             0x2001080400000000
                             0x0000000000800000";

    let instructions = program_src.split('\n');
    let mut program: Program = Program {
        instructions: Vec::new(),
        trace: Default::default(),
    };
    debug!("instructions:{:?}", program.instructions);

    for inst in instructions.into_iter() {
        program.instructions.push(inst.clone().parse().unwrap());
    }

    let mut process = Process::new();
    process.execute(&mut program).unwrap();

    println!("vm trace: {:?}", program.trace);
    let trace_json_format = serde_json::to_string(&program.trace).unwrap();

    let mut file = File::create("mul_trace.txt").unwrap();
    file.write_all(trace_json_format.as_ref()).unwrap();
}

#[test]
fn fibo_use_loop_decode() {
    // mov r0 8
    // mov r1 1
    // mov r2 1
    // mov r3 0
    // EQ r0 r3
    // cjmp 19
    // add r4 r1 r2
    // mov r1 r2
    // mov r2 r4
    // mov r4 1
    // add r3 r3 r4
    // jmp 8
    // end
    let program_src = "0x4000000840000000
        0x8
        0x4000001040000000
        0x1
        0x4000002040000000
        0x1
        0x4000004040000000
        0x0
        0x0020800100000000
        0x4000000010000000
        0x13
        0x0040408400000000
        0x0000401040000000
        0x0001002040000000
        0x4000008040000000
        0x1
        0x0101004400000000
        0x4000000020000000
        0x8
        0x0000000000800000";

    let instructions = program_src.split('\n');
    let mut program: Program = Program {
        instructions: Vec::new(),
        trace: Default::default(),
    };
    debug!("instructions:{:?}", program.instructions);

    for inst in instructions.into_iter() {
        program.instructions.push(inst.clone().parse().unwrap());
    }

    let mut process = Process::new();
    process.execute(&mut program).unwrap();

    println!("vm trace: {:?}", program.trace);
    let trace_json_format = serde_json::to_string(&program.trace).unwrap();

    let mut file = File::create("fibo_trace.txt").unwrap();
    file.write_all(trace_json_format.as_ref()).unwrap();
}

#[test]
fn memory_test() {
    // mov r0 8
    // mstore  0x100 r0
    // mov r1 2
    // mstore  0x200 r1
    // mov r0 20
    // mload r1 0x100
    // mload r2 0x200
    // mload r3 0x200
    // add r0 r1 r1
    // end
    let program_src = "0x4000000840000000
                            0x8
                            0x4020000001000000
                            0x100
                            0x4000001040000000
                            0x2
                            0x4040000001000000
                            0x200
                            0x4000000840000000
                            0x14
                            0x4000001002000000
                            0x100
                            0x4000002002000000
                            0x200
                            0x4000004002000000
                            0x200
                            0x0040200c00000000
                            0x0000000000800000";

    let instructions = program_src.split('\n');
    let mut program: Program = Program {
        instructions: Vec::new(),
        trace: Default::default(),
    };
    debug!("instructions:{:?}", program.instructions);

    for inst in instructions.into_iter() {
        program.instructions.push(inst.clone().parse().unwrap());
    }

    let mut process = Process::new();
    process.execute(&mut program).unwrap();

    println!("vm trace: {:?}", program.trace);
    let trace_json_format = serde_json::to_string(&program.trace).unwrap();

    let mut file = File::create("memory_trace.txt").unwrap();
    file.write_all(trace_json_format.as_ref()).unwrap();
}

#[test]
fn range_check_test() {
    //mov r0 8
    //mov r1 2
    //mov r2 3
    //add r3 r0 r1
    //mul r4 r3 r2
    //range_check r4
    //end
    let program_src = "0x4000000840000000
        0x8
        0x4000001040000000
        0x2
        0x4000002040000000
        0x3
        0x0020204400000000
        0x0100408200000000
        0x0001000000400000
        0x0000000000800000";

    let instructions = program_src.split('\n');
    let mut program: Program = Program {
        instructions: Vec::new(),
        trace: Default::default(),
    };
    debug!("instructions:{:?}", program.instructions);

    for inst in instructions.into_iter() {
        program.instructions.push(inst.clone().parse().unwrap());
    }

    let mut process = Process::new();
    process.execute(&mut program).unwrap();

    println!("vm trace: {:?}", program.trace);
    let trace_json_format = serde_json::to_string(&program.trace).unwrap();

    let mut file = File::create("range_check_trace.txt").unwrap();
    file.write_all(trace_json_format.as_ref()).unwrap();
}

#[test]
fn bitwise_test() {
    //mov r0 8
    //mov r1 2
    //mov r2 3
    //add r3 r0 r1
    //mul r4 r3 r2
    //and r5 r4 r3
    //or r6 r1 r4
    //xor r7 r5 r2
    //or r3 r2 r3
    //and r4 r4 r3
    //end
    let program_src = "0x4000000840000000
        0x8
        0x4000001040000000
        0x2
        0x4000002040000000
        0x3
        0x0020204400000000
        0x0100408200000000
        0x0200810000200000
        0x0041020000100000
        0x0400440000080000
        0x0080804000100000
        0x0200808000200000
        0x0000000000800000";

    let instructions = program_src.split('\n');
    let mut program: Program = Program {
        instructions: Vec::new(),
        trace: Default::default(),
    };
    debug!("instructions:{:?}", program.instructions);

    for inst in instructions.into_iter() {
        program.instructions.push(inst.clone().parse().unwrap());
    }

    let mut process = Process::new();
    process.execute(&mut program).unwrap();

    println!("vm trace: {:?}", program.trace);
    let trace_json_format = serde_json::to_string(&program.trace).unwrap();

    let mut file = File::create("bitwise_trace.txt").unwrap();
    file.write_all(trace_json_format.as_ref()).unwrap();
}

#[test]
fn comparison_test() {
    //mov r0 8
    //mov r1 2
    //mov r2 3
    //add r3 r0 r1
    //mul r4 r3 r2
    //gte r4 r3
    //cjmp 11
    //add r3 r0 r2
    //mul r4 r3 r0
    //end
    let program_src = "0x4000000840000000
        0x8
        0x4000001040000000
        0x2
        0x4000002040000000
        0x3
        0x0020204400000000
        0x0100408200000000
        0x0200800000010000
        0x4000000010000000
        0xc
        0x0020404400000000
        0x0100108200000000
        0x0000000000800000";

    let instructions = program_src.split('\n');
    let mut program: Program = Program {
        instructions: Vec::new(),
        trace: Default::default(),
    };
    debug!("instructions:{:?}", program.instructions);

    for inst in instructions.into_iter() {
        program.instructions.push(inst.clone().parse().unwrap());
    }

    let mut process = Process::new();
    process.execute(&mut program).unwrap();

    println!("vm trace: {:?}", program.trace);
    let trace_json_format = serde_json::to_string(&program.trace).unwrap();

    let mut file = File::create("comparison_trace.txt").unwrap();
    file.write_all(trace_json_format.as_ref()).unwrap();
}

#[test]
fn call_test() {
    //JMP 7
    //MUL r4 r0 10
    //ADD r4 r4 r1
    //MOV r0 r4
    //RET
    //MOV r0 8
    //MOV r1 2
    //mov r8 0x100010000
    //add r7 r8 -2
    //mov r6 0x100000000
    //mstore r7 r6
    //CALL 2
    //ADD r0 r0 r1
    //END
    let program_src = "0x4000000020000000
                             0x7
                            0x4020008200000000
                            0xa
                            0x0200208400000000
                            0x0001000840000000
                            0x0000000004000000
                            0x4000000840000000
                            0x8
                            0x4000001040000000
                            0x2
                            0x4000080040000000
                            0x100010000
                            0x6000040400000000
                            0xfffffffeffffffff
                            0x4000020040000000
                            0x100000000
                            0x0808000001000000
                            0x4000000008000000
                            0x2
                            0x0020200c00000000
                            0x0000000000800000";
    let instructions = program_src.split('\n');
    let mut program: Program = Program {
        instructions: Vec::new(),
        trace: Default::default(),
    };
    debug!("instructions:{:?}", program.instructions);

    for inst in instructions.into_iter() {
        program.instructions.push(inst.clone().parse().unwrap());
    }

    let mut process = Process::new();
    process.execute(&mut program).unwrap();

    println!("vm trace: {:?}", program.trace);
    let trace_json_format = serde_json::to_string(&program.trace).unwrap();

    let mut file = File::create("call_trace.txt").unwrap();
    file.write_all(trace_json_format.as_ref()).unwrap();
}

#[test]
fn fibo_use_loop_memory_decode() {
    //2 0 mov r0 1
    //2 2 mov r2 1
    //2 4 mstore 128 r0
    //2 6 mstore 135 r0
    //2 8 mov r0 test_loop
    //2 10 mov r3 0
    //1 12 EQ r0 r3
    //2 13 cjmp 30
    //2 15 mload  r1  128
    //1 17 assert r1  r2
    //2 18 mload  r2  135
    //1 20 add r4 r1 r2
    //2 21 mstore 128 r2
    //2 23 mstore 135 r4
    //2 25 mov r4 1
    //1 27 add r3 r3 r4
    //2 28 jmp 12
    //1 30 range_check r3
    //1 31 end
    let program_src = format!(
        "0x4000000840000000
        0x1
         0x4000002040000000
        0x1
        0x4020000001000000
        0x80
        0x4020000001000000
        0x87
        0x4000000840000000
        {:#x}
        0x4000004040000000
        0x0
        0x0020800100000000
        0x4000000010000000
        0x1e
        0x4000001002000000
        0x80
        0x0040400080000000
        0x4000002002000000
        0x87
        0x0040408400000000
        0x4080000001000000
        0x80
        0x4200000001000000
        0x87
        0x4000008040000000
        0x1
        0x0101004400000000
        0x4000000020000000
        0xc
        0x0000800000400000
        0x0000000000800000",
        0x6000
    );
    debug!("program_src:{:?}", program_src);

    let instructions = program_src.split('\n');
    let mut program: Program = Program {
        instructions: Vec::new(),
        trace: Default::default(),
    };
    debug!("instructions:{:?}", program.instructions);

    for inst in instructions.into_iter() {
        program.instructions.push(inst.clone().parse().unwrap());
    }

    let mut process = Process::new();
    let start = Instant::now();
    process.execute(&mut program).unwrap();
    let exec_time = start.elapsed();
    println!(
        "exec_time: {}, exec steps: {}",
        exec_time.as_millis(),
        program.trace.exec.len()
    );

    println!(
        "vm trace steps: {:?}, memory len: {}",
        program.trace.exec.len(),
        program.trace.memory.len()
    );
    // let trace_json_format = serde_json::to_string(&program.trace).unwrap();
    //
    // let mut file = File::create("fibo_memory.txt").unwrap();
    // file.write_all(trace_json_format.as_ref()).unwrap();
}

#[test]
fn fibo_use_loop_decode_bench() {
    // mov r0 8
    // mov r1 1
    // mov r2 1
    // mov r3 0
    // EQ r0 r3
    // cjmp 24
    // add r4 r1 r2
    // mov r1 r2
    // mov r2 r4
    // mov r4 1
    // mov r5 1
    // mov r6 2
    // add r6 r6 r5
    // add r3 r3 r4
    // jmp 8
    // end
    let program_src = "0x4000000840000000
        0x8
        0x4000001040000000
        0x1
        0x4000002040000000
        0x1
        0x4000004040000000
        0x0
        0x0020800100000000
        0x4000000010000000
        0x18
        0x0040408400000000
        0x0000401040000000
        0x0001002040000000
        0x4000008040000000
        0x1
        0x4000010040000000
        0x1
        0x4000020040000000
        0x2
        0x0802020400000000
        0x0101004400000000
        0x4000000020000000
        0x8
        0x0000000000800000";

    let instructions = program_src.split('\n');
    let mut program: Program = Program {
        instructions: Vec::new(),
        trace: Default::default(),
    };
    debug!("instructions:{:?}", program.instructions);

    for inst in instructions.into_iter() {
        program.instructions.push(inst.clone().parse().unwrap());
    }

    let mut process = Process::new();
    let start = Instant::now();
    process.execute(&mut program).unwrap();
    let exec_time = start.elapsed();
    println!(
        "exec_time: {}, exec steps: {}",
        exec_time.as_secs(),
        program.trace.exec.len()
    );
    let file = File::create("fibo_loop.txt").unwrap();

    serde_json::to_writer(file, &program.trace).unwrap();
}

#[test]
fn fibo_recursive() {
    //  main:
    //.LBL0_0:
    //  add r8 r8 4
    //  mstore [r8,-2] r8
    //  mov r1 10
    //  call fib_recursive
    //  not r7 4
    //  add r7 r7 1
    //  add r8 r8 r7
    //  end
    //  fib_recursive:
    //.LBL1_0:
    //  add r8 r8 9
    //  mstore [r8,-2] r8
    //  mov r0 r1
    //  mstore [r8,-7] r0
    //  mload r0 [r8,-7]
    //  eq r0 1
    //  cjmp .LBL1_1
    //  jmp .LBL1_2
    //.LBL1_1:
    //  mov r0 1
    //  not r7 9
    //  add r7 r7 1
    //  add r8 r8 r7
    //  ret
    //.LBL1_2:
    //  mload r0 [r8,-7]
    //  eq r0 2
    //  cjmp .LBL1_3
    //  jmp .LBL1_4
    //.LBL1_3:
    //  mov r0 1
    //  not r7 9
    //  add r7 r7 1
    //  add r8 r8 r7
    //  ret
    //.LBL1_4:
    //  not r7 1
    //  add r7 r7 1
    //  mload r0 [r8,-7]
    //  add r1 r0 r7
    //  call fib_recursive
    //  mstore [r8,-3] r0
    //  not r7 2
    //  add r7 r7 1
    //  mload r0 [r8,-7]
    //  add r0 r0 r7
    //  mstore [r8,-5] r0
    //  mload r1 [r8,-5]
    //  call fib_recursive
    //  mload r1 [r8,-3]
    //  add r0 r1 r0
    //  mstore [r8,-6] r0
    //  mload r0 [r8,-6]
    //  not r7 9
    //  add r7 r7 1
    //  add r8 r8 r7
    //  ret
    let program_src = "0x6000080400000000
                             0x4
                             0x4000020000040000
                             0x2
                             0x4800020400000000
                             0x1
                             0x2004020400000000
                             0x2004000001000000
                             0x4000001040000000
                             0xa
                             0x4000000008000000
                             0x12
                             0x4000040000040000
                             0x4
                             0x5000040400000000
                             0x1
                             0x2008080400000000
                             0x0000000000800000
                             0x6000080400000000
                             0x9
                             0x4000020000040000
                             0x2
                             0x4800020400000000
                             0x1
                             0x2004020400000000
                             0x2004000001000000
                             0x0000200840000000
                             0x4000020000040000
                             0x7
                             0x4800020400000000
                             0x1
                             0x2004020400000000
                             0x0024000001000000
                             0x4000020000040000
                             0x7
                             0x4800020400000000
                             0x1
                             0x2004020400000000
                             0x0004000802000000
                             0x4020000100000000
                             0x1
                             0x4000000010000000
                             0x2d
                             0x4000000020000000
                             0x35
                             0x4000000840000000
                             0x1
                             0x4000040000040000
                             0x9
                             0x5000040400000000
                             0x1
                             0x2008080400000000
                             0x0000000004000000
                             0x4000020000040000
                             0x7
                             0x4800020400000000
                             0x1
                             0x2004020400000000
                             0x0004000802000000
                             0x4020000100000000
                             0x2
                             0x4000000010000000
                             0x41
                             0x4000000020000000
                             0x49
                             0x4000000840000000
                             0x1
                             0x4000040000040000
                             0x9
                             0x5000040400000000
                             0x1
                             0x2008080400000000
                             0x0000000004000000
                             0x4000040000040000
                             0x1
                             0x5000040400000000
                             0x1
                             0x4000020000040000
                             0x7
                             0x4800020400000000
                             0x1
                             0x2004020400000000
                             0x0004000802000000
                             0x0028001400000000
                             0x4000000008000000
                             0x12
                             0x4000020000040000
                             0x3
                             0x4800020400000000
                             0x1
                             0x2004020400000000
                             0x0024000001000000
                             0x4000040000040000
                             0x2
                             0x5000040400000000
                             0x1
                             0x4000020000040000
                             0x7
                             0x4800020400000000
                             0x1
                             0x2004020400000000
                             0x0004000802000000
                             0x0028000c00000000
                             0x4000020000040000
                             0x5
                             0x4800020400000000
                             0x1
                             0x2004020400000000
                             0x0024000001000000
                             0x4000020000040000
                             0x5
                             0x4800020400000000
                             0x1
                             0x2004020400000000
                             0x0004001002000000
                             0x4000000008000000
                             0x12
                             0x4000020000040000
                             0x3
                             0x4800020400000000
                             0x1
                             0x2004020400000000
                             0x0004001002000000
                             0x0040100c00000000
                             0x4000020000040000
                             0x6
                             0x4800020400000000
                             0x1
                             0x2004020400000000
                             0x0024000001000000
                             0x4000020000040000
                             0x6
                             0x4800020400000000
                             0x1
                             0x2004020400000000
                             0x0004000802000000
                             0x4000040000040000
                             0x9
                             0x5000040400000000
                             0x1
                             0x2008080400000000
                             0x0000000004000000";

    let instructions = program_src.split('\n');
    let mut program: Program = Program {
        instructions: Vec::new(),
        trace: Default::default(),
    };
    debug!("instructions:{:?}", program.instructions);

    for inst in instructions.into_iter() {
        program.instructions.push(inst.clone().parse().unwrap());
    }

    let mut process = Process::new();
    process.execute(&mut program).unwrap();

    println!("vm trace: {:?}", program.trace);
    let trace_json_format = serde_json::to_string(&program.trace).unwrap();

    let mut file = File::create("fibo_recursive.txt").unwrap();
    file.write_all(trace_json_format.as_ref()).unwrap();
}
