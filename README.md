# HerKules L4 作业 · DM-MC02 USB 调试器

## MCU 是什么，程序怎么装进去

**MCU（Microcontroller Unit，微控制器）**把 CPU、程序存储器 Flash、运行时内存 SRAM 和 GPIO、UART、USB 等外设集成在一颗芯片里。**STM32** 是 STMicroelectronics 的微控制器系列，**STM32H723VG** 是 DM-MC02 板上使用的具体芯片。烧录是把编译后的固件写入 Flash。板子正常复位后，MCU 从 Flash 启动并运行这份固件。

USB-C 提供连接，板子当前运行的程序决定电脑能通过 USB 做什么。本作业固件正常启动时，USB 提供调试探针和串口功能。要通过 USB 给这颗 STM32 本身烧录，可以先让它启动 ST 预装在系统存储器里的 **Bootloader（引导程序）**。进入 **USB DFU（Device Firmware Upgrade）** 模式后，电脑可以用支持 DFU 的工具将固件写入 Flash。STM32H723 支持这条路径，前提是板子接出了对应的 USB 信号并进入系统引导模式。[ST AN2606，STM32H72xxx/73xxx](https://www.st.com/resource/en/application_note/an2606-stm32-microcontroller-system-memory-boot-mode-stmicroelectronics.pdf)

本作业首次烧录使用**外置调试探针**。电脑上的 `probe-rs` 通过外置探针和板上的 SWD 口写入 DM-MC02 的 Flash。烧好以后，DM-MC02 就能通过自己的 GPIO 和 SWD 读取目标芯片的调试端口 ID。

## 我们要做什么

假设 STM32 程序运行到一半，某个变量突然变成了 0。你可以用电脑上的**调试软件**在那行代码设断点，让目标芯片执行到那里时暂停，再检查变量、寄存器或内存。probe-rs 和 OpenOCD 都可以通过调试探针访问目标芯片。

完整的 **USB 调试探针**位于电脑和目标芯片之间。电脑把调试命令通过 USB 发给探针。探针用两个 GPIO 产生 SWD 信号，把请求送到目标芯片，再把结果传回电脑。

```text
电脑（probe-rs / OpenOCD）
        ↕ USB 上的 CMSIS-DAP v2
USB 调试探针
        ↕ 两根 SWD 信号线
目标 STM32（芯片内置的调试端口）
```

| 名称 | 在这条路径中的作用 |
| --- | --- |
| **CMSIS-DAP v2** | 电脑和探针之间的 USB 命令格式，例如请求探针读取一个调试寄存器。 |
| **SWD（Serial Wire Debug）** | 探针和目标芯片之间的通信接口。探针用 `SWCLK` 提供时钟，双方轮流驱动 `SWDIO` 来发送数据。 |
| **DAP（Debug Access Port）** | 目标芯片内置的调试硬件。它接收 SWD 请求，并提供访问芯片调试功能的入口。 |

作业用 **DM-MC02 演示这条路径的两个部分**。先用 `probe-rs list` 找到 CMSIS-DAP v2 演示接口。这个接口只提供设备发现所需的信息。再通过控制串口发 `SWD ID`，让 DM-MC02 实际产生 SWD 信号并读取目标的 **DPIDR**（Debug Port Identification Register，调试端口识别寄存器）。可把两个 GPIO 回接到同一块板的 SWD 口完成这项测试，CPU 会继续运行 USB 固件。

同一根 USB-C 数据线也提供两个串口。UART 串口转发原始字节，CAN/控制串口接收 `PING`、`SWD ID` 和 CAN 收发命令。

## 准备工作

### 1. 安装工具

先完成 [Lecture 0 环境配置](https://training.herkules.dev/learn/lecture-0)。之前装好的 Git、Rust 和系统编译工具可以继续使用，这次不需要 ROS 2。建议使用 VS Code，并安装 `rust-lang.rust-analyzer` 扩展。

打开终端，先检查：

```sh
git --version
rustup --version
cargo --version
```

然后额外安装 **probe-rs**，用于通过调试探针烧录固件：

```sh
cargo install probe-rs-tools --locked
probe-rs --version
```

安装包叫 `probe-rs-tools`，运行的命令叫 `probe-rs`。安装完成后如果找不到命令，先重新打开终端。Linux 需要按 probe-rs 说明安装 udev 规则才能不用 `sudo` 访问探针；Windows 连接探针若遇到驱动问题，按 [probe-rs 官方说明](https://probe.rs/docs/getting-started/installation/) 和助教提供的探针型号处理。

主机测试脚本还需要 Python 3 和 `pyserial`。建议在工程目录建一个虚拟环境（`.venv` 已在 `.gitignore` 中）：

```sh
python3 -m venv .venv
source .venv/bin/activate
python3 -m pip install pyserial
```

Windows PowerShell 中激活命令为 `.venv\Scripts\Activate.ps1`。之后每次打开新终端运行 `host.py` 前都要先激活。

### 2. 用模板创建自己的仓库

1. 打开 [作业模板](https://github.com/HKUAstar/embedded-training-template)。
2. 点击 **Use this template → Create a new repository**，在自己的账号下创建作业仓库。
3. 进入新仓库，点击 **Code**，复制自己的仓库地址。
4. clone 自己的仓库，再用 VS Code 打开。例如，若新仓库叫 `embedded-training`：

```sh
git clone https://github.com/YOUR_USERNAME/embedded-training.git
cd embedded-training
code .
```

将 `YOUR_USERNAME` 和仓库名换成自己的。不要直接 clone 教师模板后向原仓库提交。开发时定期 commit、push。

### 3. 第一次编译

在工程根目录执行：

```sh
cargo build --locked
```

不需要手动 `rustup target add`。rustup 会按 `rust-toolchain.toml` 获取固定工具链和 `thumbv7em-none-eabihf`（Cortex-M7）目标，首次需要联网。若固定版本无法下载，保留报错交给助教核对，不要自行删除版本配置。`--locked` 表示严格使用 `Cargo.lock` 中的依赖版本。

模板在补全 `src/wiring.rs` 前就能编译。编译产物是 `target/thumbv7em-none-eabihf/debug/dm-mc02-cmsis-dap`，这是不带扩展名的 ELF 固件文件。

### 4. 烧录是怎么接起来的

`cargo run` 不会在电脑上运行程序。`.cargo/config.toml` 把 runner 设为：

```text
probe-rs run --chip STM32H723VGTx --protocol swd ...
```

所以 `cargo run --locked` 会先编译，再调用 probe-rs，通过外置探针把 ELF 写入 STM32H723VG 的 Flash，复位后让程序运行，并保持一个日志会话。本固件没有输出 RTT 日志，烧录完成后终端停住不输出是正常的，按 Ctrl+C 退出即可，板子上的程序会继续运行。退出后才会释放探针，其他工具才能使用它。

接线：外置探针的 SWDIO、SWCLK、GND 接 DM-MC02 的 P1 SWD 口（P1.3 是 SWCLK，P1.4 是 SWDIO），按实物核对接插件方向。板子需要单独供电，例如通过 USB-C 连接电脑。烧录前先断开 SWD 回接跳线。

先确认电脑识别到外置探针：

```sh
probe-rs list
```

列表中应该有助教提供的探针型号。如果 DM-MC02 已经烧过本作业固件并接着 USB-C，列表里还会出现 `DM-MC02 CMSIS-DAP Demo`。这个演示接口只回答设备发现，不能用来烧录。probe-rs 要求选择探针时，选外置探针；也可以直接按 `probe-rs list` 显示的 VID:PID 指定：

```sh
probe-rs run --probe VID:PID --chip STM32H723VGTx --protocol swd target/thumbv7em-none-eabihf/debug/dm-mc02-cmsis-dap
```

常见问题：

| 现象 | 先检查 |
| --- | --- |
| `probe-rs list` 没有任何探针 | 探针 USB 线、驱动或 Linux udev 规则 |
| 找到探针但连不上芯片 | SWD 接线方向、GND 是否共地、板子是否上电 |
| 提示探针被占用 | 是否还有一个 `cargo run` / probe-rs 会话或 VS Code 调试器没有退出 |

## 完成两项配置

在 `src/wiring.rs` 补全两个 TODO。

1. **SWD GPIO**。从板卡上选两个可接线的 3.3 V GPIO。SWCLK 用初始电平为低的 `Output`，SWDIO 用可切换方向的 `Flex`。记录 MCU 引脚和接插件脚号。可以从外露的 PWM 信号脚里选。
2. **UART**。配置 TX、RX、中断和静态收发缓冲区，使用 115200 baud、8N1。有 P16 转接板可用 UART8。也可选板上独立的 UART7 接口，按原理图确认信号脚和方向。检查所选引脚与 SWD 是否冲突。

[作业参考](references/README.md)提供达妙原理图和使用说明书的原件链接，以及 GPIO 和 Buffered UART 示例。

## 已提供的内容

- `src/board.rs` 提供 RCC、USB 和 CAN1 的配置。Classical CAN 使用 80 MHz 内核时钟、1 Mbit/s 和 75% 采样点。Prescaler 为 1，TSEG1 为 59 tq，TSEG2 与 SJW 均为 20 tq。
- `src/dap_usb.rs` 提供 CMSIS-DAP v2 的 USB 枚举和少量 `DAP_Info` 应答。完成 GPIO 配置前，就可以检查 USB 是否列出两个串口和演示探针。[Arm DAP_Info 文档](https://arm-software.github.io/CMSIS-DAP/latest/group__DAP__Info.html)
- `src/swd.rs` 提供读取 DPIDR 所需的 GPIO 时序。补全 SWD GPIO 后，用控制串口的 `SWD ID` 测试。GPIO 尚未配置时，命令回复 `ERR SWD_NOT_CONFIGURED`。

## 烧录和验收

完成 `src/wiring.rs` 后，按[准备工作](#4-烧录是怎么接起来的)接好外置探针并烧录：

```sh
cargo build --locked
cargo run --locked
```

看到烧录完成后按 Ctrl+C 退出 probe-rs。之后断电并拔掉外置探针。将选定的 SWCLK 和 SWDIO 接到目标板对应信号。可以用同一块 DM-MC02 的 P1 做自测。P1.3 是 SWCLK，P1.4 是 SWDIO。同板只接两根信号线。两板测试则接 SWCLK、SWDIO 和 GND，两板分别供电。按实物核对接插件方向，信号线接信号脚。

通过 USB-C 重新连接电脑。先激活准备工作中建好的 Python 虚拟环境。用 `python3 -m serial.tools.list_ports` 查看串口，用 `PING` 确认控制口。将下面的 `CONTROL_PORT` 和 `UART_PORT` 换成实际串口名。

```sh
probe-rs list
python3 host.py CONTROL_PORT ping
python3 host.py CONTROL_PORT swd
python3 host.py CONTROL_PORT swd
python3 host.py CONTROL_PORT ping
python3 host.py UART_PORT uart-loopback
```

检查 `probe-rs list` 是否列出 `DM-MC02 CMSIS-DAP Demo`，两次 DPIDR 是否一致，以及最后一次 `PING` 是否正常。作业的标准探针接口只验收发现，实际 SWD 读取用 `SWD ID`。UART 测试前用跳线短接所选 UART 的 TX 和 RX。脚本会逐字节核对回环数据。

## 提交

交 `src/wiring.rs`、引脚表和实测记录。引脚表写清 SWCLK、SWDIO、UART TX 和 RX 的 MCU 引脚与接插件脚号。记录测试命令、结果和未测项目，并说明一项你用实测确认的配置。

选做可用现成的 Classical CAN 与第二个节点交换帧，或继续尝试 CAN FD 和 Linux SocketCAN。
