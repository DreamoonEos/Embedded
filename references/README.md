# 作业参考

## DM-MC02 板卡

| 资料 | 从哪里开始 |
| --- | --- |
| [原理图 V1.0](https://github.com/dmBots/DM-MC02/blob/master/drawings/2d/schematics/CtrBoard-H7_V1.0-240124.pdf) | PDF 第 1 页找 P1 SWD，第 2 页找 P11 PWM 信号和独立 UART 接口，第 6 页找 P16 扩展口。顺着信号网名找到 MCU 引脚。 |
| [使用说明书 V1.1](https://github.com/dmBots/DM-MC02/blob/master/manual/%E8%BE%BE%E5%A6%99%E7%A7%91%E6%8A%80DM-MC-Board02%E7%94%B5%E6%9C%BA%E5%BC%80%E5%8F%91%E6%9D%BF%E4%BD%BF%E7%94%A8%E8%AF%B4%E6%98%8E%E4%B9%A6V1.1.pdf) | 查第 7 节 PWM、第 8 节串口、第 9 节 SWD 和第 17 节用户扩展 IO。用图示定位接插件，并按原理图核对板上脚号。 |

## Embassy 0.6.0

| 作业步骤 | 上游示例或源码 |
| --- | --- |
| GPIO 输出 | [STM32H7 blinky 示例](https://github.com/embassy-rs/embassy/blob/84444a19eb57d978e3c09fcdc8e60cdd7278eb03/examples/stm32h7/src/bin/blinky.rs)。看 GPIO 初始化和设置电平。 |
| 同一 GPIO 切换输入与输出 | [GPIO 源码中的 `Flex`](https://github.com/embassy-rs/embassy/blob/84444a19eb57d978e3c09fcdc8e60cdd7278eb03/embassy-stm32/src/gpio.rs)。看 `Flex::new`、`set_as_input`、`set_as_output`。 |
| 中断驱动的 UART | [Buffered UART 示例](https://github.com/embassy-rs/embassy/blob/84444a19eb57d978e3c09fcdc8e60cdd7278eb03/examples/stm32f4/src/bin/usart_buffered.rs)。看中断绑定、缓冲区和配置。示例用 STM32F4，引脚和中断需要按 DM-MC02 查找。 |
| 核对 UART 参数顺序 | [`BufferedUart` 源码](https://github.com/embassy-rs/embassy/blob/84444a19eb57d978e3c09fcdc8e60cdd7278eb03/embassy-stm32/src/usart/buffered.rs)。对照构造函数，核对 RX/TX 引脚和 TX/RX 缓冲区顺序。 |

也可参考 [Embassy 的 STM32H723VG 引脚资料](https://docs.embassy.dev/embassy-stm32/git/stm32h723vg/index.html)。本作业使用 Cargo.lock 锁定的版本。
