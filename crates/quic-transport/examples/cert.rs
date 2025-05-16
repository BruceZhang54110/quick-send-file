use std::fs;

/// 证书生成示例
fn main() {

    let subject_alt_names = vec!["localhost".to_string()];

    let certified_key =rcgen::generate_simple_self_signed(subject_alt_names).unwrap();
    // 生成证书文件
    fs::write("server.cert", certified_key.cert.pem().as_bytes()).unwrap();
    // 生成私钥文件
    fs::write("server.key", certified_key.key_pair.serialize_pem()).unwrap();

}