package com.coloryr.mml.serversync;

import java.nio.file.Path;

/** 命令行选项。 */
final class Options {
    String manifest;
    Path gameDir;
    Path baseDir;
    Path stateDir;
    /** 日志文件；为空时用状态目录下的默认文件名 */
    Path logFile;
    String apiKey = "";
    boolean force;
    boolean dryRun;
    boolean noDelete;
    boolean quiet;
    boolean help;
    boolean version;
}
