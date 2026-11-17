//! mixin / ASM 目标扫描的单元测试
//!
//! 类文件是按格式手搓的：只放常量池、两个方法与类级注解，够走通解析就行。

use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use super::{ModObj, asm, mixin};

/// 按 名字 -> 内容 造一个内存 jar
fn build_zip(files: &[(&str, Vec<u8>)]) -> ZipArchive<Cursor<Vec<u8>>> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    for (name, data) in files {
        writer.start_file(*name, options).unwrap();
        writer.write_all(data).unwrap();
    }
    let cursor = writer.finish().unwrap();
    ZipArchive::new(cursor).unwrap()
}

/// 往常量池里塞一条 UTF8，返回索引
fn push_utf8(pool: &mut Vec<Vec<u8>>, value: &str) -> u16 {
    let mut bytes = vec![1u8];
    bytes.extend_from_slice(&(value.len() as u16).to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
    pool.push(bytes);
    pool.len() as u16
}

/// 往常量池里塞一条 Class，返回索引
fn push_class(pool: &mut Vec<Vec<u8>>, name_index: u16) -> u16 {
    let mut bytes = vec![7u8];
    bytes.extend_from_slice(&name_index.to_be_bytes());
    pool.push(bytes);
    pool.len() as u16
}

/// 一条注解的字节：单个注解、按 (元素名, 值) 给元素对
fn annotation_body(type_index: u16, values: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&1u16.to_be_bytes());
    body.extend_from_slice(&type_index.to_be_bytes());
    body.extend_from_slice(&(values.len() as u16).to_be_bytes());
    for (name, value) in values {
        body.extend_from_slice(&name.to_be_bytes());
        body.extend(value);
    }
    body
}

/// 字符串元素值，标签 s
fn string_value(index: u16) -> Vec<u8> {
    let mut value = vec![b's'];
    value.extend_from_slice(&index.to_be_bytes());
    value
}

/// 空的 Code 属性体：max_stack / max_locals / 空指令 / 空异常表 / 空属性表
fn code_body() -> Vec<u8> {
    vec![0; 12]
}

/// 写一条属性
fn push_attribute(bytes: &mut Vec<u8>, name: u16, body: &[u8]) {
    bytes.extend_from_slice(&name.to_be_bytes());
    bytes.extend_from_slice(&(body.len() as u32).to_be_bytes());
    bytes.extend_from_slice(body);
}

/// 写一个方法
fn push_method(bytes: &mut Vec<u8>, name: u16, desc: u16, attrs: &[(u16, Vec<u8>)]) {
    bytes.extend_from_slice(&0x0001u16.to_be_bytes());
    bytes.extend_from_slice(&name.to_be_bytes());
    bytes.extend_from_slice(&desc.to_be_bytes());
    bytes.extend_from_slice(&(attrs.len() as u16).to_be_bytes());
    for (attr_name, body) in attrs {
        push_attribute(bytes, *attr_name, body);
    }
}

/// 造一个带 `@Mixin`、`@Inject` 与 `@Overwrite` 的类
fn mixin_class_bytes() -> Vec<u8> {
    let mut pool: Vec<Vec<u8>> = Vec::new();
    let this_name = push_utf8(&mut pool, "test/Mixin");
    let this_class = push_class(&mut pool, this_name);
    let super_name = push_utf8(&mut pool, "java/lang/Object");
    let super_class = push_class(&mut pool, super_name);
    let mixin_desc = push_utf8(&mut pool, "Lorg/spongepowered/asm/mixin/Mixin;");
    let targets_name = push_utf8(&mut pool, "targets");
    let target_class = push_utf8(&mut pool, "net.minecraft.client.Minecraft");
    let tick_name = push_utf8(&mut pool, "onTick");
    let void_desc = push_utf8(&mut pool, "()V");
    let inject_desc = push_utf8(&mut pool, "Lorg/spongepowered/asm/mixin/injection/Inject;");
    let method_name = push_utf8(&mut pool, "method");
    let target_method = push_utf8(&mut pool, "m_12345_");
    let overwrite_desc = push_utf8(&mut pool, "Lorg/spongepowered/asm/mixin/Overwrite;");
    let render_name = push_utf8(&mut pool, "onRender");
    let annotations = push_utf8(&mut pool, "RuntimeVisibleAnnotations");
    let code = push_utf8(&mut pool, "Code");

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[0xCA, 0xFE, 0xBA, 0xBE]);
    bytes.extend_from_slice(&0u16.to_be_bytes());
    bytes.extend_from_slice(&52u16.to_be_bytes());
    bytes.extend_from_slice(&((pool.len() + 1) as u16).to_be_bytes());
    for entry in &pool {
        bytes.extend_from_slice(entry);
    }

    bytes.extend_from_slice(&0x0021u16.to_be_bytes());
    bytes.extend_from_slice(&this_class.to_be_bytes());
    bytes.extend_from_slice(&super_class.to_be_bytes());
    bytes.extend_from_slice(&0u16.to_be_bytes());
    bytes.extend_from_slice(&0u16.to_be_bytes());

    bytes.extend_from_slice(&2u16.to_be_bytes());
    let inject = annotation_body(inject_desc, &[(method_name, string_value(target_method))]);
    push_method(
        &mut bytes,
        tick_name,
        void_desc,
        &[(annotations, inject), (code, code_body())],
    );
    let overwrite = annotation_body(overwrite_desc, &[]);
    push_method(
        &mut bytes,
        render_name,
        void_desc,
        &[(annotations, overwrite), (code, code_body())],
    );

    let mixin = annotation_body(mixin_desc, &[(targets_name, string_value(target_class))]);
    bytes.extend_from_slice(&1u16.to_be_bytes());
    push_attribute(&mut bytes, annotations, &mixin);

    bytes
}

#[test]
fn mixin_annotations_are_read() {
    let mut zip = build_zip(&[
        (
            "test.mixins.json",
            br#"{"package":"test","mixins":["Mixin"]}"#.to_vec(),
        ),
        ("test/Mixin.class", mixin_class_bytes()),
    ]);
    let mut info = ModObj::default();
    mixin::read_mixins(&mut zip, &mut info);

    assert_eq!(info.mixins.len(), 2, "{:?}", info.mixins);
    let inject = info
        .mixins
        .iter()
        .find(|item| item.kind == "inject")
        .expect("没读出 @Inject");
    assert_eq!(inject.config, "test.mixins.json");
    assert_eq!(inject.mixin, "test.Mixin");
    assert_eq!(inject.target, "net.minecraft.client.Minecraft");
    assert_eq!(inject.method, "m_12345_");

    let overwrite = info
        .mixins
        .iter()
        .find(|item| item.kind == "overwrite")
        .expect("没读出 @Overwrite");
    assert_eq!(overwrite.method, "onRender", "@Overwrite 用方法自己的名字");
}

#[test]
fn coremod_js_and_manifest_are_read() {
    let js = br#"function initializeCoreMod() {
    return {
        'ench': {
            'target': {
                'type': 'METHOD',
                'class': 'net.minecraft.world.item.ItemStack',
                'methodName': 'getRarity'
            }
        }
    }
}"#;
    let mut zip = build_zip(&[
        ("META-INF/coremods.json", br#"{"ench":"coremods/ench.js"}"#.to_vec()),
        ("coremods/ench.js", js.to_vec()),
        (
            "META-INF/MANIFEST.MF",
            b"Manifest-Version: 1.0\nFMLCorePlugin: com.example.Plugin\n".to_vec(),
        ),
        (
            "META-INF/services/cpw.mods.modlauncher.api.ITransformationService",
            b"com.example.Service\n".to_vec(),
        ),
    ]);
    let mut info = ModObj::default();
    asm::read_asm(&mut zip, &mut info);

    assert_eq!(info.asm.len(), 3, "{:?}", info.asm);
    let coremod = info
        .asm
        .iter()
        .find(|item| item.source == "coremods.json")
        .expect("没读出 coremods.json");
    assert_eq!(coremod.transformer, "ench");
    assert_eq!(coremod.target, "net.minecraft.world.item.ItemStack");
    assert_eq!(coremod.method, "getRarity");

    let plugin = info
        .asm
        .iter()
        .find(|item| item.source == "FMLCorePlugin")
        .expect("没读出 FMLCorePlugin");
    assert_eq!(plugin.transformer, "com.example.Plugin");
    assert!(plugin.target.is_empty(), "旧版插件没有可读的目标");

    assert!(
        info.asm
            .iter()
            .any(|item| item.transformer == "com.example.Service")
    );
}