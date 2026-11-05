package com.coloryr.mml.serversync;

import java.nio.file.Path;

/** 命令行选项。 */
final class Options {
    String manifest;
    Path gameDir;
    Path baseDir;
    Path stateDir;
    String apiKey = "";
    boolean force;
    boolean dryRun;
    boolean noDelete;
    boolean quiet;
    boolean help;
    boolean version;
}
