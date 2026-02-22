import Cocoa
import Foundation

private struct LauncherConfig {
    let host: String
    let port: Int
    let configPath: String
}

private var wizardProcess: Process?
private var wizardLogHandle: FileHandle?

private final class Logger {
    private let handle: FileHandle?

    init(logPath: URL) {
        FileManager.default.createFile(atPath: logPath.path, contents: nil)
        self.handle = try? FileHandle(forWritingTo: logPath)
        self.handle?.seekToEndOfFile()
    }

    func log(_ message: String) {
        let stamp = ISO8601DateFormatter().string(from: Date())
        let line = "[\(stamp)] \(message)\n"
        if let data = line.data(using: .utf8) {
            try? handle?.write(contentsOf: data)
        }
    }
}

private func loadLauncherConfig() -> LauncherConfig {
    let info = Bundle.main.infoDictionary ?? [:]
    let host = (info["FarmSetupHost"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines)
    let port = info["FarmSetupPort"] as? Int
    let configPath = (info["FarmSetupConfigPath"] as? String)?
        .trimmingCharacters(in: .whitespacesAndNewlines)

    return LauncherConfig(
        host: (host?.isEmpty == false) ? host! : "127.0.0.1",
        port: port ?? 8800,
        configPath: (configPath?.isEmpty == false) ? configPath! : "/Users/Shared/FarmDashboard/setup/config.json"
    )
}

private func httpOk(_ url: URL, timeout: TimeInterval = 0.6) -> Bool {
    var request = URLRequest(url: url)
    request.httpMethod = "GET"
    request.timeoutInterval = timeout

    let semaphore = DispatchSemaphore(value: 0)
    var ok = false
    URLSession.shared.dataTask(with: request) { _, response, _ in
        if let http = response as? HTTPURLResponse {
            ok = (200..<300).contains(http.statusCode)
        }
        semaphore.signal()
    }.resume()
    _ = semaphore.wait(timeout: .now() + timeout + 0.2)
    return ok
}

private func findEmbeddedControllerDmg(in resources: URL) throws -> URL {
    let fm = FileManager.default
    let entries = try fm.contentsOfDirectory(at: resources, includingPropertiesForKeys: nil)
    if let match = entries.first(where: { $0.pathExtension == "dmg" && $0.lastPathComponent.contains("FarmDashboardController") }) {
        return match
    }
    throw NSError(domain: "FarmDashboardInstaller", code: 2, userInfo: [
        NSLocalizedDescriptionKey: "Missing embedded controller DMG in app resources"
    ])
}

private func copyReplacing(_ src: URL, _ dst: URL) throws {
    let fm = FileManager.default
    if fm.fileExists(atPath: dst.path) {
        try fm.removeItem(at: dst)
    }
    try fm.copyItem(at: src, to: dst)
}

private func chmodExecutable(_ path: URL) {
    _ = chmod(path.path, 0o755)
}

private func stripQuarantine(_ path: URL) {
    let task = Process()
    task.executableURL = URL(fileURLWithPath: "/usr/bin/xattr")
    task.arguments = ["-d", "com.apple.quarantine", path.path]
    task.standardOutput = FileHandle.nullDevice
    task.standardError = FileHandle.nullDevice
    try? task.run()
    task.waitUntilExit()
}

private func launchWizard(
    farmctl: URL,
    bundleDmg: URL,
    config: LauncherConfig,
    runtimeRoot: URL
) throws -> (Process, FileHandle) {
    let fm = FileManager.default
    try? fm.createDirectory(at: runtimeRoot, withIntermediateDirectories: true)

    let logPath = runtimeRoot.appendingPathComponent("farmctl-setup.log")
    fm.createFile(atPath: logPath.path, contents: nil)
    let logHandle = try FileHandle(forWritingTo: logPath)

    let process = Process()
    process.executableURL = farmctl
    process.arguments = [
        "--profile", "prod",
        "serve",
        "--host", config.host,
        "--port", String(config.port),
        "--config", config.configPath,
        "--no-auto-open",
    ]
    var env = ProcessInfo.processInfo.environment
    env["FARM_SETUP_BOOTSTRAP"] = "1"
    env["FARM_SETUP_BUNDLE_PATH"] = bundleDmg.path
    process.environment = env
    process.standardOutput = logHandle
    process.standardError = logHandle
    try process.run()
    return (process, logHandle)
}

private func main() {
    let config = loadLauncherConfig()
    let fm = FileManager.default
    let runtimeRoot = fm.temporaryDirectory.appendingPathComponent("farm_dashboard_installer", isDirectory: true)
    try? fm.createDirectory(at: runtimeRoot, withIntermediateDirectories: true)
    let logger = Logger(logPath: runtimeRoot.appendingPathComponent("launcher.log"))

    guard let resourcesPath = Bundle.main.resourcePath else {
        logger.log("error: Bundle.main.resourcePath was nil")
        return
    }
    logger.log("resources: \(resourcesPath)")

    let baseUrl = URL(string: "http://\(config.host):\(config.port)")!
    if httpOk(baseUrl.appendingPathComponent("healthz")) {
        logger.log("wizard already running; opening \(baseUrl)")
        NSWorkspace.shared.open(baseUrl)
        return
    }

    let resourcesUrl = URL(fileURLWithPath: resourcesPath, isDirectory: true)
    logger.log("runtime: \(runtimeRoot.path)")

    do {
        let farmctlSrc = resourcesUrl.appendingPathComponent("farmctl")
        let farmctlDst = runtimeRoot.appendingPathComponent("farmctl")
        logger.log("copy farmctl: \(farmctlSrc.path) -> \(farmctlDst.path)")
        try copyReplacing(farmctlSrc, farmctlDst)
        chmodExecutable(farmctlDst)
        stripQuarantine(farmctlDst)

        let controllerSrc = try findEmbeddedControllerDmg(in: resourcesUrl)
        let controllerDst = runtimeRoot.appendingPathComponent(controllerSrc.lastPathComponent)
        logger.log("copy bundle: \(controllerSrc.path) -> \(controllerDst.path)")
        try copyReplacing(controllerSrc, controllerDst)
        stripQuarantine(controllerDst)

        logger.log("starting farmctl serve on \(baseUrl)")
        let (process, logHandle) = try launchWizard(
            farmctl: farmctlDst,
            bundleDmg: controllerDst,
            config: config,
            runtimeRoot: runtimeRoot
        )
        wizardProcess = process
        wizardLogHandle = logHandle

        process.terminationHandler = { proc in
            logger.log("farmctl serve exited (status=\(proc.terminationStatus))")
            DispatchQueue.main.async {
                wizardLogHandle?.closeFile()
                exit(Int32(proc.terminationStatus))
            }
        }
    } catch {
        logger.log("error: \(error.localizedDescription)")
        let alert = NSAlert()
        alert.messageText = "Farm Dashboard Installer failed to start"
        alert.informativeText = "See installer logs at: \(runtimeRoot.path)/launcher.log"
        alert.alertStyle = .critical
        alert.runModal()
        return
    }

    DispatchQueue.global(qos: .userInitiated).async {
        for _ in 0..<60 {
            if httpOk(baseUrl.appendingPathComponent("healthz"), timeout: 0.4) {
                logger.log("wizard healthy; opening \(baseUrl)")
                DispatchQueue.main.async {
                    NSWorkspace.shared.open(baseUrl)
                }
                return
            }
            usleep(250_000)
        }
        logger.log("wizard did not become healthy within timeout")
        DispatchQueue.main.async {
            let alert = NSAlert()
            alert.messageText = "Farm Dashboard Installer started but the wizard did not become ready"
            alert.informativeText = "See installer logs at: \(runtimeRoot.path)/launcher.log"
            alert.alertStyle = .warning
            alert.runModal()
            exit(2)
        }
    }

    RunLoop.main.run()
}

main()
