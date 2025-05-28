# quick-send-file

quick-send-file 是一个使用 Rust 编程语言开发的端到端文件发送工具，基于稳定的 UDP 协议构建。
## 预期目标

- **高效传输**：利用 QUIC 协议的多路复用和低延迟特性，实现高效的文件传输。
- **端到端加密**：确保文件传输过程中的数据安全。
- **跨平台支持**：支持在多种操作系统上运行，包括 Windows、Linux 和 macOS。
- **简单易用**：提供直观的命令行界面，快速发送和接收文件。


## 项目思路

调研要使用的网络传输工具从最初 quinn crate 到 lib-p2p crate 再到现在的 iroh crate


## 参考
学习 [iroh](https://crates.io/crates/iroh) 以及实际文件传输项目 [sendme](https://github.com/n0-computer/sendme) 搭建自己的文件传输项目。

后续会进行改写，实现更多功能

## 使用方法

发送文件：
```
cargo run -- send -p [file path]
```

接收文件：
```
cargo run -- receive --code [ticket]
```

---
