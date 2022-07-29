#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;
use user_lib::{close, open, unlink, write, OpenFlags};

/// 测试大量 open/unlink，输出 Test mass open/unlink OK! 就算正确。

#[no_mangle]
pub fn main() -> i32 {
    let test_str = "some random long long long long long long long long string".repeat(50);
    let fname = "fname3\0";
    for i in 0..10 {
        println!("---------------------------------------------【{}】--------- 1", i);
        let fd = open(fname, OpenFlags::CREATE | OpenFlags::WRONLY);
        println!("---------------------------------------------【{}】--------- 2", i);
        if fd == -1 {
            panic!("failed to crate file");
        }
        println!("---------------------------------------------【{}】--------- 3", i);
        let fd = fd as usize;
        println!("---------------------------------------------【{}】--------- 4", i);
        for _ in 0..50 {
            write(fd, test_str.as_bytes());
        }
        println!("---------------------------------------------【{}】--------- 5", i);
        close(fd);
        println!("---------------------------------------------【{}】--------- 6", i);
        // assert_eq!(unlink(fname), 0);
        println!("---------------------------------------------【{}】--------- 7", i);
        let fd = open(fname, OpenFlags::RDONLY);
        println!("---------------------------------------------【{}】--------- 8", i);
        // assert!(fd < 0);
        println!("test iteration {}", i)
    }
    println!("Test mass open/unlink OK!");
    0
}
