# emx-mbox 分析与重构计划

## 当前问题分析

### 1. Parser (MboxReader) 问题

#### 1.1 生命周期设计缺陷
- `next_message()` 返回 `ParsedMail<'_>`，借用自 `self.buffer`
- 无法同时持有多个消息的解析结果
- 应返回拥有所有权的原始字节 `Vec<u8>`，让调用者按需解析

#### 1.2 消息分隔符检测不完整
- go-mbox 支持 `From ` 行前无空行的 malformed-but-valid 格式
- 当前实现遇到 `From ` 行直接 break，不会回退空行
- go-mbox 在遇到空行时会预读下一行判断是否为分隔符，避免在消息末尾产生多余空行

#### 1.3 `>From` 反转义逻辑不正确
- mboxrd 格式支持多级 `>` 转义（`>>From`、`>>>From` 等）
- 当前只处理单层 `>From `，且反转义逻辑多余（strip_prefix 后又 format 回去）
- 正确做法：如果行匹配 `^>+From `，去掉一个前导 `>`

#### 1.4 CRLF 处理缺失
- go-mbox reader 将 CRLF 转换为 `\r\n` 输出（规范化）
- emx-mbox 没有标准化行结尾处理

#### 1.5 first_line 逻辑混乱
- `first_line` 标志目的不明确，`From ` 后第一行无条件添加

### 2. Writer (MboxWriter) 问题

#### 2.1 分隔符格式不标准
- 当前 `write_message(&str, &[u8])` 生成 `From {from}\n`
- 标准 mbox 格式: `From sender@domain Day Mon DD HH:MM:SS YYYY`
- b4 使用: `From mboxrd@z Thu Jan  1 00:00:00 1970`

#### 2.2 消息间分隔不正确
- go-mbox writer 在每条消息结束后写入 `\n\n`（两个空行）
- 当前实现最后一行不写换行，然后只写一个 `\n`

#### 2.3 CRLF → LF 转换缺失
- go-mbox writer 将输入中的 CRLF 替换为 LF 存储
- mbox 文件内部应使用 LF

### 3. 数据模型 (Mbox / MailMessage) 问题

#### 3.1 MailMessage 过度解析
- 存储 from / subject / body 解析字段，丢失了大量头部信息
- 应以原始字节为核心，按需提供头部访问方法

#### 3.2 envelope "From " 与 header "From:" 混淆
- `MailMessage.from` 存储的是 `From:` 头部值
- mbox envelope `From ` 行的发件人信息被丢弃

#### 3.3 add_message 生成的邮件不符合 RFC 5322
- 缺少 Message-ID、Date、MIME-Version、Content-Type
- 无法与 b4 配合使用

### 4. b4 兼容性差距

#### 4.1 缺少的必要头部
b4 要求每条消息必须包含:
- **Message-ID**: `<local@domain>` 格式
- **From**: RFC 5322 地址格式
- **Date**: RFC 5322 日期格式
- **Subject**: 支持 `[PATCH vN M/N]` 前缀
- **Content-Type**: `text/plain; charset=utf-8`
- **MIME-Version**: `1.0`

#### 4.2 缺少的可选头部
- **In-Reply-To**: 用于消息线程串联
- **References**: 祖先消息 ID 列表
- **To / Cc**: 收件人列表

#### 4.3 mboxrd 格式支持
- b4 优先使用 mboxrd 格式
- 分隔符: `From mboxrd@z Thu Jan  1 00:00:00 1970`
- 需要正确的多级 `>From` 转义

---

## 重构方案

### 阶段 1: 核心 Reader/Writer 重写

1. **MboxReader** 改为返回 `Vec<u8>` 原始消息字节
   - 参考 go-mbox `messageReader` 的空行预读逻辑
   - 正确处理 mboxrd `>From` 多级反转义
   - 支持 malformed-but-valid 格式（From 行前无空行）

2. **MboxWriter** 完整重写
   - 标准 `From sender date\n` 分隔符
   - 支持 mboxrd 格式（`From mboxrd@z Thu Jan  1 00:00:00 1970`）
   - 正确的 `>From` 转义
   - CRLF → LF 转换
   - 消息结尾 `\n\n` 分隔

### 阶段 2: 数据模型重构

1. **MailMessage** 以 raw bytes 为核心
   - 保留 envelope_from（From 行发件人）
   - 提供按需解析头部的方法
   - 保留 raw bytes 供 round-trip

2. **Mbox** 追加功能增强
   - `append_raw(raw_bytes)`: 追加原始 EML 数据
   - `append_eml(path)`: 从 EML 文件追加
   - `append_message(builder)`: 使用构建器追加

### 阶段 3: 邮件构建器 (MessageBuilder)

面向 b4 兼容的消息构建:
- 自动生成 Message-ID
- RFC 5322 格式日期
- 支持 [PATCH vN M/N] Subject 格式
- In-Reply-To / References 线程管理
- Content-Type / MIME-Version
- Signed-off-by 等 trailer 支持

### 阶段 4: 测试覆盖

- 多消息解析（含 malformed 格式）
- From 转义 round-trip
- EML 文件追加
- MessageBuilder 各字段
- b4 格式兼容性验证
