# HBuilder 打包全流程

## 先说结论

当前 `.android` 里的项目本质是：

- 前端：Vue 3 + Vite
- 原生能力：Tauri 2 + Rust

而 **HBuilderX** 能直接打包的是：

- uni-app 项目
- 纯前端 H5/HTML5+ 项目
- 使用 DCloud 原生插件体系接入的能力

所以这两者并不是同一种原生宿主。

### 这意味着什么

1. **你可以用 HBuilder 打包“前端页面壳”**
2. **你不能直接把当前的 Tauri/Rust 原生能力一起打进去**
3. 如果要在 HBuilder 中做成“功能完整可用”的 App，必须把这些能力改写为 HBuilder/uni-app 可用的移动端 API：
   - `@tauri-apps/api/core invoke`
   - `@tauri-apps/plugin-opener`
   - `@tauri-apps/plugin-os`
   - Tauri WebviewWindow / 多窗口能力
   - Rust 命令层
   - Rust 本地代理服务

换句话说：

- **当前 `.android/all` 这套文件，适合继续走 Tauri Android 打包**
- **如果你坚持走 HBuilder 打包，则只能先打包静态前端壳，功能不会完整**

---

## 目录说明

你现在需要看的目录是：

- `all\web`：前端源码
- `all\web\dist`：前端构建产物
- `all\core`：Rust/Tauri 核心
- `all\app`：Android 工程占位和说明
- `all\docs`：构建与打包说明
- `all\scripts`：同步脚本

---

## 方案 A：继续用 Tauri Android 打包（推荐，功能完整）

这是当前工程最匹配的方案。

### 1. 安装环境

需要本机具备：

- Node.js
- Rust
- Cargo
- Android Studio
- Android SDK
- Android NDK
- Java JDK

### 2. 安装前端依赖

```powershell
cd D:\Claude_Code\DTV mobile\.android\all\web
npm install
```

### 3. 同步 Rust core 到 `src-tauri`

```powershell
cd D:\Claude_Code\DTV mobile\.android\all\web
powershell -ExecutionPolicy Bypass -File ..\scripts\sync-core.ps1
```

### 4. 初始化 Android 工程

```powershell
cd D:\Claude_Code\DTV mobile\.android\all\web
npm run android:init
```

### 5. 把生成的 Android 工程同步到 `all\app`

```powershell
cd D:\Claude_Code\DTV mobile\.android\all\web
powershell -ExecutionPolicy Bypass -File ..\scripts\sync-generated-android.ps1
```

### 6. 本地真机/模拟器调试

```powershell
cd D:\Claude_Code\DTV mobile\.android\all\web
npm run android:dev
```

### 7. 构建安装包

```powershell
cd D:\Claude_Code\DTV mobile\.android\all\web
npm run android:build
```

---

## 方案 B：使用 HBuilderX 打包“静态前端壳”（只能演示页面，不保证功能）

这个方案只能把页面包成 App 外壳。由于 Tauri/Rust API 不存在，很多功能会失效。

### 1. 准备一个 HBuilder 项目目录

新建目录，例如：

```text
D:\Claude_Code\DTV mobile\HBuilder-DTV
```

### 2. 把 `all\web\dist` 里的内容拷进去

需要复制：

- `index.html`
- `assets\`

来源：

```text
D:\Claude_Code\DTV mobile\.android\all\web\dist
```

### 3. 在 HBuilder 项目根目录创建 `manifest.json`

最小示例：

```json
{
  "name": "DTVMobileShell",
  "appid": "__UNI__DTVMOBILE",
  "description": "DTV mobile shell",
  "versionName": "1.0.0",
  "versionCode": "100",
  "h5plus": {
    "launch_path": "index.html",
    "runmode": "liberate",
    "distribute": {
      "android": {
        "permissions": [
          "<uses-permission android:name=\"android.permission.INTERNET\"/>"
        ]
      }
    }
  }
}
```

### 4. 用 HBuilderX 打开这个目录

操作顺序：

1. 打开 HBuilderX
2. `文件 -> 打开目录`
3. 选择你的 HBuilder 项目目录
4. 确认根目录下有：
   - `index.html`
   - `assets`
   - `manifest.json`

### 5. 本地运行

在 HBuilderX 中：

1. 右键项目
2. 选择“运行到手机或模拟器”
3. 或选择“运行到内置浏览器”

### 6. 云打包 / 本地打包

在 HBuilderX 中：

1. 点击菜单“发行”
2. 选择“原生 App-云打包”
3. 选择 Android
4. 配置应用名称、包名、图标、启动图
5. 提交打包

---

## 为什么 HBuilder 打出来会不完整

因为当前页面中仍然有这类能力依赖：

- 平台搜索、分类、取流、弹幕：靠 Rust `invoke`
- 本地代理：靠 Rust 服务
- B 站 Cookie 辅助：靠 Tauri 原生能力
- 移动原生宿主接口：现在写的是 Tauri，不是 uni-app / 5+ API

所以如果你直接把 `dist` 丢进 HBuilder：

- 页面能打开
- 纯静态 UI 能显示
- 但很多核心功能会报错或失效

---

## 如果你一定要最终用 HBuilder 打功能完整包

那下一阶段必须继续做一次“宿主迁移”：

### 前端层要改

把这些 Tauri 接口全部替换掉：

- `invoke(...)`
- `@tauri-apps/plugin-opener`
- `@tauri-apps/plugin-os`
- `WebviewWindow`

### 原生层要改

把 Rust/Tauri 能力改成 HBuilder 可调用的实现之一：

1. uni-app 原生插件
2. HTML5+ Native.js
3. Android 原生插件 + JS Bridge
4. 独立后端服务接口

### 你至少要补齐这些能力

- 直播列表接口
- 搜索接口
- 房间详情接口
- 真实播放流解析
- 弹幕连接与转发
- 图片代理 / 跨域方案
- Cookie 方案

---

## 最后建议

如果目标是：

- **尽快打出能用的 Android App**

那请走 **方案 A：Tauri Android**。

如果目标是：

- **必须使用 HBuilderX 生态**

那当前 `.android/all` 可以作为迁移底稿，但还不能直接打成功能完整包。
