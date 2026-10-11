//! 着色器源码模块
//!
//! 内置皮肤渲染使用的 GLSL 顶点 / 片段着色器源码，
//! 通过宏兼容不同 GLSL 版本与 macOS。

/// macOS 专用的 GLSL 版本头，内容为 `#version 150` 加 `Macos` 宏定义
pub const MACOS_HEADER: &str = r#"#version 150
#define Macos
"#;

/// 顶点着色器源码，变换坐标并传出 UV、法线与世界坐标
pub const VERTEX_SHADER_SOURCE: &str = r#"#if __VERSION__ >= 130
#define COMPAT_VARYING out
#define COMPAT_ATTRIBUTE in
#define COMPAT_TEXTURE texture
#else
#define COMPAT_VARYING varying
#define COMPAT_ATTRIBUTE attribute
#define COMPAT_TEXTURE texture2D
#endif

COMPAT_ATTRIBUTE vec3 a_position;
COMPAT_ATTRIBUTE vec2 a_texCoord;
COMPAT_ATTRIBUTE vec3 a_normal;

uniform mat4 model;
uniform mat4 projection;
uniform mat4 view;
uniform mat4 self;

COMPAT_VARYING vec3 normalIn;
COMPAT_VARYING vec2 texIn;
COMPAT_VARYING vec3 fragPosIn;

void main()
{
    texIn = a_texCoord;

    mat4 temp = view * model * self;

    fragPosIn = vec3(model * vec4(a_position, 1.0));
    normalIn = normalize(vec3(model * vec4(a_normal, 1.0)));
    gl_Position = projection * temp * vec4(a_position, 1.0);
}"#;

/// 片段着色器源码，按环境光加漫反射的简单光照模型采样贴图
pub const FRAGMENT_SHADER_SOURCE: &str = r#"#if defined(GL_ES)
precision mediump float;
#endif

#ifdef Macos
#define COMPAT_VARYING in
#define COMPAT_ATTRIBUTE in
#define COMPAT_TEXTURE texture
out vec4 FragColor;
#else
#if __VERSION__ >= 130
#define COMPAT_VARYING in
#define COMPAT_ATTRIBUTE in
#define COMPAT_TEXTURE texture
out vec4 FragColor;
#else
#define COMPAT_VARYING varying
#define COMPAT_ATTRIBUTE attribute
#define COMPAT_TEXTURE texture2D
#define FragColor gl_FragColor
#endif
#endif

uniform sampler2D texture0;

COMPAT_VARYING vec3 fragPosIn;
COMPAT_VARYING vec3 normalIn;
COMPAT_VARYING vec2 texIn;

void main() {
    vec3 lightColor = vec3(1.0, 1.0, 1.0);
    float ambientStrength = 0.15;
    vec3 lightPos = vec3(0, 1, 5);
    
    vec3 ambient = ambientStrength * lightColor;
    vec3 norm = normalize(normalIn);
    vec3 lightDir = normalize(lightPos - fragPosIn);
    float diff = max(dot(norm, lightDir), 0.0);
    vec3 diffuse = diff * lightColor;
    
    vec3 result = (ambient + diffuse);
    FragColor = COMPAT_TEXTURE(texture0, texIn) * vec4(result, 1.0);
}"#;
