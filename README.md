## 项目简介

本项目以 Starry-On-ArceOS 的 [oscomp](https://github.com/Azure-stars/Starry-On-ArceOS/tree/oscomp) 分支 和 Starry 的 [monolithic](https://github.com/Azure-stars/Starry/tree/monolithic)（其为 [arceos](https://github.com/arceos-org/arceos) 的 fork） 为基础，继续将 Unikernel 扩充为宏内核，扩充到支持全部初赛测例。具体修改在[e34716404ba4a75e22a8853ce476b7919dece120](https://gitlab.eduxiji.net/T202410459994713/oskernel2024-46/-/commit/e34716404ba4a75e22a8853ce476b7919dece120) 提交及之后。

Starry/monolithic 是一个组件化的 Unikernel 基座，Starry-On-ArceOS/oscomp 是对 Starry/monolithic 的宏内核扩展。其中，扩展主要是通过额外的指针字段、Rust 的宏、条件编译等手段实现的。

## 参考开源项目

[Starry/main](https://github.com/Azure-stars/Starry/)

## 比赛方向

小型内核实现

## 运行方式

先克隆本项目，然后进入项目目录，执行以下命令：

```shell
make

qemu-system-riscv64 -machine virt -kernel kernel-qemu -m 128M -nographic -smp 2 -bios default -drive file=sdcard.img,if=none,format=raw,id=x0  -device virtio-blk-device,drive=x0 -device virtio-net-device,netdev=net -netdev user,id=net
```

如遇环境问题，可使用评测镜像：

```shell
docker pull docker.educg.net/cg/os-contest:2024p8.3

# 在项目根目录下执行
docker run -it -v .:/app docker.educg.net/cg/os-contest:2024p8.3 /bin/bash 

cd /app
```

然后再运行之前的命令。

## 项目结构

- 📄 Cargo.lock
- 📄 Cargo.toml
- 📄 Makefile
- 📄 README.md
- 🗂️ arceos - Unikernel 基座，fork 自 [Starry/monolithic](https://github.com/Azure-stars/Starry/tree/monolithic)
  - 📄 Cargo.lock
  - 📄 Cargo.toml
  - 📄 Makefile
  - 📄 README.md
  - 🗂️ api - 与内核空间相关的接口
    - 🗂️ arceos_posix_api - 一些POSIX API，有些用来实现 Linux 系统调用
    - 🗂️ axfeat - 统一转发 feature，防止 feature 混乱
  - 🗂️ doc
  - 🗂️ modules
    - 🗂️ axalloc - 全局内存分配器
    - 🗂️ axconfig - 特定平台编译的常量和参数配置
    - 🗂️ axdriver - 设备驱动模块
    - 🗂️ axfs - 文件系统模块
    - 🗂️ axhal - 硬件抽象层
    - 🗂️ axlog - 日志模块
    - 🗂️ axmm - 内存管理模块
    - 🗂️ axns - 命名空间模块（与linux不同）
    - 🗂️ axruntime - 运行时库，是应用程序运行的基础环境
    - 🗂️ axsync - 同步操作模块，提供Mutex等
    - 🗂️ axtask - 任务调度管理模块
  - 🗂️ platforms - 不同架构的配置文件（与内核空间相关）
  - 📄 rust-toolchain.toml - Rust 工具链配置文件
  - 🗂️ scripts
- 📄 build.rs - 根据构建目标、以及不同架构对用户空间的配置文件，生成Rust可直接使用的文件uspace_config.rs
- 🗂️ configs - 不同架构对用户空间的配置文件
- 🗂️ doc
- 🖼️ kernel-qemu
- 🖼️ kernel-qemu.elf
- 🗂️ scripts
- 💾 sdcard.img
- 🗂️ src - Unikernel 的 宏内核扩展
  - 📄 main.rs - 宏内核扩展入口文件
  - 📄 loader.rs - elf加载器
  - 📄 mm.rs - 内存管理
  - 📄 task.rs - 宏内核下对Unikernel任务的扩展
  - 🗂️ task - 同上
    - 📄 heap.rs - 堆内存管理
    - 📄 time.rs - 时间统计
  - 🗂️ syscall_imp - 系统调用实现
    - 🗂️ fs - 文件系统相关的系统调用
    - 🗂️ mm - 内存有关的系统调用
    - 📄 mod.rs - 中转系统调用
    - 📄 system_info.rs - 系统信息相关的系统调用
    - 🗂️ task  - 任务调度相关的系统调用
    - 📄 time.rs - 时间相关的系统调用
- 🗂️ target
- 🗂️ vendor (项目依赖的外来库们)
- 🖼️ vfat12.img - 用于通过(u)mount测例的vfat文件系统镜像

## 文档

[系统调用实现文档](doc/main_syscalls_implementation.md)

可参考本项目所基于的 Starry/monolithic 下的文档 [arceos/doc](arceos/doc)，以及[简明 ArceOS Tutorial Book](https://rcore-os.cn/arceos-tutorial-book/)。

原项目的 README.md 请查看 [doc/README.md](doc/README.md)。

## 演示视频

链接: <https://pan.baidu.com/s/1gB_cGPAb6wVlU3GKMO5EWA?pwd=mp11>
