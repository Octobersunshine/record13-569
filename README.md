# 3D 模型上传与轻量化减面压缩服务

基于 Rust + Axum 的 Web 服务，提供 3D 模型文件上传、自动发起异步减面压缩任务。

## 功能特性

- **文件上传 (multipart/form-data)
- **支持 OBJ、STL 格式
- **基于 QEM (Quadric Error Metrics) 减面算法
- **异步任务队列与进度跟踪
- **RESTful API 设计
- **任务状态查询与结果下载

## 技术栈

- **Web 框架**: Axum 0.7
- **异步运行时**: Tokio
- **模型加载**: tobj (OBJ), stl_io (STL)
- **减面算法**: QEM (自实现)
- **序列化**: Serde

## 项目结构

```
src/
├── main.rs          # 主入口
├── lib.rs           # 库模块声明
├── config.rs        # 配置模块
├── model.rs         # 数据模型 (API 类型)
├── error.rs         # 错误类型
├── uploader.rs      # 文件上传处理
├── compressor.rs    # 3D 模型压缩核心 (QEM 算法)
├── task.rs          # 任务管理
└── routes.rs        # 路由定义
```

## API 接口

### 健康检查
```
GET /health
```

### 上传模型并发起压缩
```
POST /api/upload
Content-Type: multipart/form-data

字段:
  file                # 3D 模型文件 (必填)
  quality             # 压缩质量 0.01-1.0 (可选, 默认 0.5)
  target_face_count  # 目标面数 (可选)
  target_vertex_count # 目标顶点数 (可选)
  preserve_borders   # 是否保留边界 (可选)
  preserve_uvs       # 是否保留 UV (可选)
```

响应:
```json
{
  "task_id": "uuid",
  "filename": "model.obj",
  "original_size_bytes": 123456,
  "options": { ... }
}
```

### 查询任务列表
```
GET /api/tasks
```

### 查询任务状态
```
GET /api/tasks/:task_id
```

响应:
```json
{
  "task_id": "uuid",
  "status": "processing | completed | failed | queued",
  "progress": 0.75,
  "original_filename": "model.obj",
  "original_info": { "vertex_count": 10000, "face_count": 20000, "file_size_bytes": 123456 },
  "compressed_info": { ... },
  "download_url": "/api/tasks/uuid/download"
}
```

### 下载压缩结果
```
GET /api/tasks/:task_id/download
```

## 环境变量

| 变量 | 默认值 | 说明
|------|--------|------
| SERVER_PORT | 8080 | 服务端口
| UPLOAD_DIR | ./uploads | 上传目录
| OUTPUT_DIR | ./outputs | 输出目录
| MAX_UPLOAD_SIZE_MB | 512 | 最大上传大小
| DEFAULT_QUALITY | 0.5 | 默认压缩质量
| ALLOWED_EXTENSIONS | obj,stl | 允许的扩展名，逗号分隔

## 启动

```bash
# 检查代码
cargo check

# 开发模式运行
cargo run

# 生产构建 (需要完整的工具链)
cargo build --release
```

## 注意: Windows GNU 工具链需要安装完整的 MinGW-w64 (包含 dlltool.exe。建议使用 MSYS2 或安装完整。
```bash
# 使用测试文件上传
curl -X POST http://localhost:8080/api/upload \
  -F "file=@tests/cube.obj" \
  -F "quality=0.5"
```
