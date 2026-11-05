package com.coloryr.mml.serversync;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** 一次服务器实例更新。 */
final class Sync {
    private final Options options;
    private final Downloader downloader;
    private final Path tempDir;

    Sync(Options options) {
        this.options = options;
        this.downloader = new Downloader(options.apiKey, options.quiet);
        this.tempDir = options.stateDir.resolve("tmp");
    }

    void run() throws IOException {
        String manifestUrl = toUrl(options.manifest);
        byte[] data = downloader.fetch(manifestUrl);
        Manifest current = Manifest.parse(data);
        current.resolveUrls(manifestUrl);

        Manifest old = loadOld(current);
        if (!options.force && sameVersion(old, current)) {
            info("包版本未变（" + current.packVersion + "），跳过同步");
            return;
        }

        validate(current);
        if (old != null) {
            validate(old);
        }

        List<String> removedDirs = new ArrayList<>();
        Set<String> removedFiles = new HashSet<>();
        planRemovals(old, current, removedFiles, removedDirs);
        if (options.dryRun) {
            printPlan(current, old, removedFiles, removedDirs);
            return;
        }
        if (!options.noDelete) {
            applyRemovals(removedFiles, removedDirs);
        } else {
            info("--no-delete：跳过 " + removedFiles.size() + " 个文件、" + removedDirs.size() + " 个目录的删除");
        }

        boolean needBase = current.base != null && (old == null || old.base == null);
        List<Manifest.FileEntry> files = changedFiles(current);
        List<ArchivePlan> archives = archivePlans(old, current, removedDirs);
        int archiveCount = 0;
        for (ArchivePlan plan : archives) {
            if (!plan.skip) {
                archiveCount++;
            }
        }
        int total = files.size() + archiveCount + (needBase ? 1 : 0);
        info("需要处理 " + total + " 项（模组/文件 " + files.size() + "，配置压缩包 "
                + archiveCount + "，基础包 " + (needBase ? 1 : 0) + "）");

        Files.createDirectories(tempDir);
        Path baseTemp = null;
        int done = 0;
        if (needBase) {
            Manifest.BaseEntry base = current.base;
            baseTemp = tempDir.resolve("base-" + safeName(base.file));
            downloader.download(base.url, baseTemp, base.sha1, base.sha256);
            info("下载基础包完成，解压到 " + options.baseDir);
            done++;
            progress(done, total);
        }

        for (Manifest.FileEntry item : files) {
            Path target = Fs.resolveSafe(options.gameDir, item.path);
            String url = downloader.resolveFile(item);
            downloader.download(url, target, item.sha1, item.sha256);
            done++;
            progress(done, total);
        }

        List<ArchivePlan> downloaded = new ArrayList<>();
        for (ArchivePlan plan : archives) {
            if (plan.skip) {
                continue;
            }
            Path temp = tempDir.resolve("archive-" + done + "-" + safeName(plan.item.file));
            String url = plan.item.url.trim();
            if (url.isEmpty()) {
                throw new IOException("配置压缩包没有下载地址: " + plan.item.file);
            }
            downloader.download(url, temp, plan.item.sha1, plan.item.sha256);
            downloaded.add(new ArchivePlan(plan.item, temp, false));
            done++;
            progress(done, total);
        }

        if (baseTemp != null) {
            Fs.extractZip(baseTemp, options.baseDir, true);
            Files.deleteIfExists(baseTemp);
        }

        downloaded.sort(Comparator.comparingInt((ArchivePlan plan) -> depth(plan.item.dir)));
        for (ArchivePlan plan : downloaded) {
            Path target = Fs.resolveSafe(options.gameDir, plan.item.dir);
            if (plan.item.deleteOld) {
                Fs.deleteDirectory(target);
            }
            Fs.extractZip(plan.temp, target, false);
            Files.deleteIfExists(plan.temp);
            info("解压配置: " + plan.item.dir);
        }

        saveState(data);
        info("同步完成，当前包版本 " + current.packVersion);
    }

    private Manifest loadOld(Manifest current) {
        Path state = options.stateDir.resolve("last-manifest.json");
        Path fallback = options.baseDir.resolve("server.json");
        for (Path path : new Path[]{state, fallback}) {
            if (!Files.isRegularFile(path)) {
                continue;
            }
            try {
                byte[] data = Files.readAllBytes(path);
                Manifest old = Manifest.parse(data);
                old.resolveUrls(toUrl(options.manifest));
                return old;
            } catch (IOException e) {
                info("忽略无法读取的旧清单 " + path + "：" + e.getMessage());
            }
        }
        return null;
    }

    private boolean sameVersion(Manifest old, Manifest current) {
        if (old == null) {
            return false;
        }
        String a = old.packVersion.trim();
        String b = current.packVersion.trim();
        return !b.isEmpty() && a.equals(b);
    }

    private void validate(Manifest manifest) throws IOException {
        for (Manifest.FileEntry item : manifest.files) {
            Fs.resolveSafe(options.gameDir, item.path);
        }
        for (Manifest.ArchiveEntry item : manifest.archives) {
            Fs.resolveSafe(tempDir, item.file);
            Fs.resolveSafe(options.gameDir, item.dir);
        }
        if (manifest.base != null) {
            Fs.resolveSafe(tempDir, manifest.base.file);
        }
    }

    private void planRemovals(Manifest old, Manifest current, Set<String> files,
                              List<String> dirs) {
        if (old == null) {
            return;
        }
        Map<String, Manifest.FileEntry> byIdentity = new HashMap<>();
        for (Manifest.FileEntry item : current.files) {
            byIdentity.put(item.identity(), item);
        }
        for (Manifest.FileEntry item : old.files) {
            Manifest.FileEntry replacement = byIdentity.get(item.identity());
            if (replacement == null || !replacement.path.equals(item.path)) {
                files.add(item.path);
            }
        }
        Set<String> currentArchives = new HashSet<>();
        for (Manifest.ArchiveEntry item : current.archives) {
            currentArchives.add(item.file);
        }
        for (Manifest.ArchiveEntry item : old.archives) {
            if (!currentArchives.contains(item.file) && item.deleteOld && !item.dir.isEmpty()) {
                dirs.add(item.dir);
            }
        }
        dirs.sort(Comparator.comparingInt((String dir) -> -depth(dir)));
    }

    private void applyRemovals(Set<String> files, List<String> dirs) throws IOException {
        for (String path : files) {
            Path target = Fs.resolveSafe(options.gameDir, path);
            Fs.deleteFile(target);
            info("删除旧文件: " + path);
        }
        for (String dir : dirs) {
            Path target = Fs.resolveSafe(options.gameDir, dir);
            if (Files.exists(target)) {
                Fs.deleteDirectory(target);
                info("删除旧配置目录: " + dir);
            }
        }
    }

    private List<Manifest.FileEntry> changedFiles(Manifest current) throws IOException {
        List<Manifest.FileEntry> out = new ArrayList<>();
        for (Manifest.FileEntry item : current.files) {
            Path target = Fs.resolveSafe(options.gameDir, item.path);
            if (!downloader.matches(target, item.sha1, item.sha256)) {
                out.add(item);
            }
        }
        return out;
    }

    private List<ArchivePlan> archivePlans(Manifest old, Manifest current, List<String> removedDirs) {
        Map<String, Manifest.ArchiveEntry> oldArchives = new HashMap<>();
        if (old != null) {
            for (Manifest.ArchiveEntry item : old.archives) {
                oldArchives.put(item.file, item);
            }
        }
        Set<String> skipped = new HashSet<>();
        for (Manifest.ArchiveEntry item : current.archives) {
            if (archiveUnchanged(oldArchives.get(item.file), item)) {
                skipped.add(item.dir);
            }
        }
        Set<String> cleared = new HashSet<>(removedDirs);
        for (Manifest.ArchiveEntry item : current.archives) {
            if (!skipped.contains(item.dir) && item.deleteOld) {
                cleared.add(item.dir);
            }
        }
        List<ArchivePlan> out = new ArrayList<>();
        for (Manifest.ArchiveEntry item : current.archives) {
            boolean skip = skipped.contains(item.dir);
            if (skip) {
                for (String ancestor : cleared) {
                    if (Fs.under(item.dir, ancestor)) {
                        skip = false;
                        break;
                    }
                }
            }
            out.add(new ArchivePlan(item, null, skip));
        }
        return out;
    }

    private boolean archiveUnchanged(Manifest.ArchiveEntry old, Manifest.ArchiveEntry current) {
        if (old == null || !old.dir.equals(current.dir) || old.deleteOld != current.deleteOld) {
            return false;
        }
        String a = old.sha1.isEmpty() ? old.sha256 : old.sha1;
        String b = current.sha1.isEmpty() ? current.sha256 : current.sha1;
        if (a.isEmpty() || !a.equalsIgnoreCase(b)) {
            return false;
        }
        try {
            Path target = Fs.resolveSafe(options.gameDir, current.dir);
            return Files.isDirectory(target);
        } catch (IOException e) {
            return false;
        }
    }

    private void printPlan(Manifest current, Manifest old, Set<String> removedFiles,
                          List<String> removedDirs) {
        info("清单: " + (current.name.isEmpty() ? "(未命名)" : current.name)
                + "，版本 " + current.packVersion);
        info("将删除旧文件 " + removedFiles.size() + " 个");
        for (String path : removedFiles) {
            info("  - " + path);
        }
        info("将删除旧配置目录 " + removedDirs.size() + " 个");
        for (String dir : removedDirs) {
            info("  - " + dir);
        }
        info("当前需要检查 " + current.files.size() + " 个文件，"
                + current.archives.size() + " 个配置压缩包");
        if (current.base != null && (old == null || old.base == null)) {
            info("基础包: " + current.base.file);
        }
    }

    private void saveState(byte[] data) throws IOException {
        Files.createDirectories(options.stateDir);
        Path target = options.stateDir.resolve("last-manifest.json");
        Path temp = options.stateDir.resolve("last-manifest.json.tmp");
        Files.write(temp, data);
        try {
            Files.move(temp, target, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
        } catch (java.nio.file.AtomicMoveNotSupportedException e) {
            Files.move(temp, target, StandardCopyOption.REPLACE_EXISTING);
        }
        Files.deleteIfExists(temp);
    }

    private void progress(int done, int total) {
        if (!options.quiet && total > 0) {
            System.out.println("[progress] " + done + "/" + total);
        }
    }

    private void info(String message) {
        if (!options.quiet) {
            System.out.println("[info] " + message);
        }
    }

    private static int depth(String path) {
        int count = 0;
        for (int i = 0; i < path.length(); i++) {
            if (path.charAt(i) == '/') {
                count++;
            }
        }
        return count;
    }

    private static String safeName(String name) {
        String out = name.replace('\\', '/');
        int slash = out.lastIndexOf('/');
        if (slash >= 0) {
            out = out.substring(slash + 1);
        }
        return out.replaceAll("[^A-Za-z0-9._-]", "_");
    }

    private static String toUrl(String value) throws IOException {
        String text = value.trim();
        if (text.matches("^[a-zA-Z][a-zA-Z0-9+.-]*:.*")) {
            return text;
        }
        return java.nio.file.Paths.get(text).toAbsolutePath().normalize().toUri().toString();
    }

    private static final class ArchivePlan {
        final Manifest.ArchiveEntry item;
        final Path temp;
        final boolean skip;

        ArchivePlan(Manifest.ArchiveEntry item, Path temp, boolean skip) {
            this.item = item;
            this.temp = temp;
            this.skip = skip;
        }
    }
}
