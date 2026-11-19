![spektrafilm-rs](docs/card.jpg)

# spektrafilm-rs

[English](README.md) | **简体中文**

spektrafilm-rs 把数码照片当作一次胶片曝光，模拟显影，然后把它印到相纸上，或直接扫描胶片。它是摄影应用 **FPW** 使用的独立 Rust 引擎，也可以单独使用。你可以用命令行处理照片，在桌面界面预览，或把引擎接入自己的应用。FPW 自行固定引擎版本并选择开放的控件。

## 现在能用什么

| 环节 | 已接通的能力 |
| --- | --- |
| 曝光与显影 | 彩色负片、反转片与黑白胶片的光谱曝光和显影，手动曝光、测光与高光／暗部保护。支持镜头扩散、光晕、DIR 层间作用，以及跟随原图坐标的多层颗粒。可选的宽容度提示能标出画面有多少接近所选胶片的亮部或暗部边界。 |
| 黑白摄影 | Tri-X 400、T-Max 100、Acros II、HP5 Plus 可印到三乳剂的 Ilford Multigrade IV RC。纸号可选 00、0、1、**2（默认）**、3、4、5。可选择实测无滤镜响应、8/12 号黄镜和 25/29 号红镜，并按胶片补偿曝光。 |
| 印相与扫描 | 光学印相支持放大机校色、曝光、预闪、相纸曲线调整及显影液耗竭。直扫胶片跳过相纸。负片漂白旁路会改变显影后的负片，印相漂白旁路则作用在彩色相纸上。 |
| 片幅与片边 | 驻留 Vulkan 支持 135 全格、半格、XPan 和可设片门尺寸的 120。配方可按胶片坐标放置边码、帧号、日期和局部漏光。[片幅与后端限制](docs/135_FILM_BORDER.md) |
| 文件与宿主接入 | 独立工具可读取 JPEG、PNG、线性 TIFF 和 RAW。CUBE 导出可烘焙逐像素的颜色处理。宿主可复用 GPU 设备、已显影负片和工作缓冲，取回 GPU、RGB16 或区域输出。CPU 提供 f64 参考构建。[用法与支持的入口](docs/USAGE.md) |

### 默认观感与可选效果

RGB 转胶片曝光默认用 **Hanatos 2025**，有拟合数据的胶片会使用自己的光谱响应窗口。**Arctic beta04** 可手动选择。Hanatos 的「逐胶片颜色校正」是独立开关，默认关闭，20 款有系数的胶片可用。

彩色负片默认走**光学印相**。负片色相重建可在彩色相纸、sRGB 输出、手动黄／品红校色都为零时开启，CPU 和驻留 Vulkan 都支持。两种漂白旁路默认强度均为零。在驻留路径上，换纸号、切换印相色相或调整印相漂白旁路都可以复用负片。调整负片漂白旁路会重新显影。[配方与控件](docs/USAGE.md)

### 成片验证

我们用实拍照片和灰阶做了印相对照。在已验证的黑白和彩色印相配方下，CPU 和 Vulkan 的输出都与参考成片接近。[验证范围与样例配方](docs/ENGINE_EVOLUTION.md)

### 哪些还用不了，为什么

| 状态 | 项目与原因 |
| --- | --- |
| 做过研究，决定暂不接入 | 去除背涂层后的背面反射原型会在高反差边缘留下过强的红色拖尾。光谱波长平滑、重建白点归一化和 Langmuir DIR 也保留为研究。正式渲染沿用现有光晕、归一化和 DIR 模型。 |
| 等待测量或授权 | Multigrade 暂无半号，现有灰阶不足以支持可靠的 4½ 号。Portra 160NC、160VC、400NC、400VC 及配套中性印相数据仍待明确再分发条款。 |
| 尚未开发 | 独立工具尚未接通内嵌 ICC 转换，更广色域的全流程改造仍是独立课题。CPU 和 CUDA 也没有物理片边渲染路径，CUDA 后端目前为实验性支持。 |

## 快速开始

需要 Rust **1.99+** 及所在平台的原生构建工具。Vulkan 是目前经过验证的物理引擎 GPU 后端。
CI 覆盖 Windows 构建和 Linux 软件 Vulkan 计算；其他平台与后端组合需要单独验证。
原生依赖、图形界面与导出配置，以及实验性 CUDA 选项，见[构建与使用](docs/USAGE.md)。

```sh
git clone https://github.com/ye602a/spektrafilm-rs.git
cd spektrafilm-rs
cargo build --locked --release -p spektrafilm-cli --bin spektrafilm
./target/release/spektrafilm list-profiles --data-dir data
./target/release/spektrafilm process input.tif -o output.png \
    --film kodak_portra_400 --paper kodak_portra_endura --data-dir data
```

Windows 下请使用 `target\release\spektrafilm.exe`，并将处理命令写在同一行。

## 文档

- [引擎批次、上游对照与实测依据](docs/ENGINE_EVOLUTION.md)
- [构建与使用](docs/USAGE.md)
- [宿主 GPU 接入](docs/maintenance/HOST_GPU_INTEGRATION_CONTRACT.md)
- [公开接入示例](crates/spektrafilm-core/examples/fpw_host_contract.rs)
- [兼容性与开发计划](docs/maintenance/FPW_PARITY_MIGRATION_PLAN.md)
- [胶片引擎实测记录](docs/ENGINE_STUDIES.md)
- [验证范围与依据](docs/maintenance/VALIDATION.md)

## 致谢与许可

最初的 Rust 实现由 [turbasvin](https://github.com/turbasvin/spektrafilm-rs) 完成。
胶片模型、光谱与配置数据来源于
[Andrea Volpato 的 spektrafilm](https://github.com/andreavolpato/spektrafilm)。
光谱升采样基于 [Johannes Hanatos](https://github.com/hanatos) 的工作。
数值计算实现参考了 NumPy、SciPy、scikit-image 和 colour-science；
[多层颗粒与高精度 IIR 的来源说明](docs/maintenance/GRAIN_MULTILAYER_MIGRATION.md)
包含 SpektraLab 的贡献。

代码采用 [GPL-3.0](LICENSE) 许可。光谱数据、配置数据及其衍生内容适用
[上游署名与 CC BY-SA 4.0 条款](SPEKTRAFILM_DATA_LICENSE.txt)。
四款黑白负片和 Multigrade IV RC 的授权来源与 RS 相纸校准另有[署名与校准记录](data/SPEKTRALAB_BW_NOTICE.md)。
[版本化校准数据的来源记录](data/negative_scan_ap0_v1/PROVENANCE.md)与数据一同保留。
