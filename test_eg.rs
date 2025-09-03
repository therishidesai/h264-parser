fn main() {
    // Test input: 0b00101000
    // This is: 00100 in exp-golomb
    // Leading zeros: 2
    // So we read 2 more bits after the 1: 00
    // code_num = (1 << 2) - 1 + 0 = 4 - 1 + 0 = 3
    
    println!("0b00101000 decodes to UE code_num = 3");
    println!("code_num 3 is odd, so SE value = (3 + 1) / 2 = 2");
    
    // Test input: 0b00101100
    // This is: 00101 in exp-golomb
    // Leading zeros: 2
    // So we read 2 more bits after the 1: 01
    // code_num = (1 << 2) - 1 + 1 = 4 - 1 + 1 = 4
    
    println!("\n0b00101100 decodes to UE code_num = 4");
    println!("code_num 4 is even, so SE value = -(4 / 2) = -2");
}
