import AppKit
import Foundation

/// One synced item shown in the "Recent" list.
struct Activity: Identifiable, Codable, Equatable {
    var id = UUID()
    let op: String          // uploaded, downloaded, deleted_local, deleted_remote, conflict
    let path: String        // relative to the local sync folder
    let isDir: Bool
    let at: Date
    var original: String?

    var name: String { (path as NSString).lastPathComponent }
    var folder: String {
        let d = (path as NSString).deletingLastPathComponent
        return d.isEmpty ? "" : d
    }
    var verb: String {
        switch op {
        case "uploaded": return "Uploaded"
        case "downloaded": return "Downloaded"
        case "deleted_local": return "Removed from this Mac"
        case "deleted_remote": return "Deleted from server"
        case "conflict": return "Conflict – kept both copies"
        default: return op.capitalized
        }
    }
    var symbol: String {
        switch op {
        case "uploaded": return "arrow.up.circle.fill"
        case "downloaded": return "arrow.down.circle.fill"
        case "conflict": return "exclamationmark.triangle.fill"
        default: return "trash.circle.fill"
        }
    }
    var isDelete: Bool { op.hasPrefix("deleted") }
}

struct Transfer: Equatable {
    var op: String
    var path: String
    var index: Int
    var total: Int
    var bytes: Int64
    var size: Int64
    var name: String { (path as NSString).lastPathComponent }
    var fraction: Double? { size > 0 ? min(1, Double(bytes) / Double(size)) : nil }
}

struct Usage: Equatable {
    var used: Int64
    var free: Int64
    var total: Int64
}

enum LoginState: Equatable {
    case idle, busy, needCode, codeRejected, success
    case failed(String, localNetwork: Bool)
}

/// Owns the Rust sync helper (`vaultix-sync agent`) and mirrors its state for the UI.
@MainActor
final class AppModel: ObservableObject {
    static let shared = AppModel()

    // Configuration
    @Published var configured = false
    @Published var server = ""
    @Published var username = ""
    @Published var localDir = ""
    @Published var remoteDir = ""
    @Published var paused = false
    @Published var version = ""
    @Published var logPath = ""
    @Published private(set) var stateKnown = false

    // Live status
    @Published var phase = "starting"
    @Published var statusLine = "Starting…"
    @Published var lastOk: Date?
    @Published var transfer: Transfer?
    @Published var recent: [Activity] = []
    @Published var usage: Usage?
    @Published var login: LoginState = .idle
    @Published var helperError: String?

    private var proc: Process?
    private var stdin: FileHandle?
    private var buffer = Data()
    private var quitting = false
    private var lastPhase = ""

    private init() {
        if let d = UserDefaults.standard.data(forKey: "recent"), let r = try? JSONDecoder().decode([Activity].self, from: d) {
            recent = r
        }
    }

    // MARK: helper process

    private var helperURL: URL? {
        if let p = ProcessInfo.processInfo.environment["VAULTIX_AGENT"] { return URL(fileURLWithPath: p) }
        return Bundle.main.url(forAuxiliaryExecutable: "vaultix-sync")
    }

    func start() {
        guard proc == nil, let url = helperURL else {
            if helperURL == nil { helperError = "The sync helper is missing from the app bundle." }
            return
        }
        let p = Process()
        p.executableURL = url
        p.arguments = ["agent"]
        let inPipe = Pipe(), outPipe = Pipe()
        p.standardInput = inPipe
        p.standardOutput = outPipe
        p.standardError = FileHandle.nullDevice
        outPipe.fileHandleForReading.readabilityHandler = { [weak self] h in
            let chunk = h.availableData
            DispatchQueue.main.async { self?.consume(chunk) }
        }
        p.terminationHandler = { [weak self] _ in
            DispatchQueue.main.async {
                guard let self else { return }
                self.proc = nil
                self.stdin = nil
                if !self.quitting {
                    // Restart the helper if it ever dies.
                    DispatchQueue.main.asyncAfter(deadline: .now() + 2) { self.start() }
                }
            }
        }
        do {
            try p.run()
            proc = p
            stdin = inPipe.fileHandleForWriting
            helperError = nil
        } catch {
            helperError = "Couldn't start the sync helper: \(error.localizedDescription)"
        }
    }

    func quit() {
        quitting = true
        send(["cmd": "quit"])
        let p = proc
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            if p?.isRunning == true { p?.terminate() }
            NSApp.terminate(nil)
        }
    }

    private func send(_ obj: [String: Any]) {
        guard let stdin, var data = try? JSONSerialization.data(withJSONObject: obj) else { return }
        data.append(0x0A)
        try? stdin.write(contentsOf: data)
    }

    private func consume(_ chunk: Data) {
        if chunk.isEmpty { return }
        buffer.append(chunk)
        while let nl = buffer.firstIndex(of: 0x0A) {
            let line = buffer.subdata(in: buffer.startIndex..<nl)
            buffer.removeSubrange(buffer.startIndex...nl)
            if let obj = try? JSONSerialization.jsonObject(with: line) as? [String: Any] {
                handle(obj)
            }
        }
    }

    // MARK: events

    private func handle(_ e: [String: Any]) {
        switch e["event"] as? String {
        case "state":
            configured = e["configured"] as? Bool ?? false
            server = e["server"] as? String ?? ""
            username = e["username"] as? String ?? ""
            localDir = e["local_dir"] as? String ?? ""
            remoteDir = e["remote_dir"] as? String ?? ""
            paused = e["paused"] as? Bool ?? false
            version = e["version"] as? String ?? ""
            logPath = e["log_path"] as? String ?? ""
            let first = !stateKnown
            stateKnown = true
            if configured { refreshUsage() }
            if first && !configured { Windows.shared.showSetup() }
        case "status":
            let newPhase = e["phase"] as? String ?? "idle"
            statusLine = e["line"] as? String ?? ""
            if let ms = e["last_ok"] as? Double { lastOk = Date(timeIntervalSince1970: ms / 1000) }
            if newPhase != "syncing" {
                pendingTransfer = nil
                if transfer != nil { transfer = nil }
            }
            if newPhase != lastPhase {
                if lastPhase == "syncing" && newPhase == "idle" { refreshUsage(); persistRecent() }
                if ["error", "signedout"].contains(newPhase) && configured {
                    Notifier.post(title: newPhase == "signedout" ? "Vaultix is signed out" : "Vaultix sync problem", body: statusLine)
                }
                lastPhase = newPhase
            }
            phase = newPhase
        case "progress":
            pendingTransfer = Transfer(
                op: e["op"] as? String ?? "",
                path: e["path"] as? String ?? "",
                index: e["index"] as? Int ?? 0,
                total: e["total"] as? Int ?? 0,
                bytes: (e["bytes"] as? NSNumber)?.int64Value ?? 0,
                size: (e["size"] as? NSNumber)?.int64Value ?? 0)
            scheduleFlush()
        case "activity":
            let a = Activity(
                op: e["op"] as? String ?? "",
                path: e["path"] as? String ?? "",
                isDir: e["is_dir"] as? Bool ?? false,
                at: Date(timeIntervalSince1970: ((e["at"] as? Double) ?? Date().timeIntervalSince1970 * 1000) / 1000),
                original: e["original"] as? String)
            pendingActivities.append(a)
            if pendingActivities.count > 40 { pendingActivities.removeFirst(pendingActivities.count - 40) }
            scheduleFlush()
            if a.op == "conflict" {
                Notifier.post(title: "Edited in two places", body: "Vaultix kept both versions of “\((a.original as NSString?)?.lastPathComponent ?? a.name)”.")
            }
        case "usage":
            usage = Usage(
                used: (e["used"] as? NSNumber)?.int64Value ?? 0,
                free: (e["free"] as? NSNumber)?.int64Value ?? 0,
                total: (e["total"] as? NSNumber)?.int64Value ?? 0)
        case "login_result":
            if e["ok"] as? Bool == true {
                login = .success
            } else if e["need_code"] as? Bool == true {
                login = codeSent ? .codeRejected : .needCode
            } else {
                login = .failed(e["error"] as? String ?? "Sign-in failed", localNetwork: e["local_network"] as? Bool ?? false)
            }
            codeSent = false
        case "fatal", "error":
            helperError = e["message"] as? String
        default:
            break
        }
    }

    // MARK: coalescing
    // A big sync emits thousands of events per second; the UI only needs a few updates a second.

    private var pendingTransfer: Transfer?
    private var pendingActivities: [Activity] = []
    private var flushScheduled = false
    private var lastPersist = Date.distantPast

    private func scheduleFlush() {
        guard !flushScheduled else { return }
        flushScheduled = true
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) { [weak self] in self?.flush() }
    }

    private func flush() {
        flushScheduled = false
        if let t = pendingTransfer {
            pendingTransfer = nil
            if transfer != t { transfer = t }
        }
        if !pendingActivities.isEmpty {
            var r = recent
            for a in pendingActivities {
                r.removeAll { $0.path == a.path }
                r.insert(a, at: 0)
            }
            pendingActivities.removeAll()
            if r.count > 40 { r.removeLast(r.count - 40) }
            recent = r
            if Date().timeIntervalSince(lastPersist) > 5 { persistRecent() }
        }
    }

    private func persistRecent() {
        lastPersist = Date()
        if let d = try? JSONEncoder().encode(recent) { UserDefaults.standard.set(d, forKey: "recent") }
    }

    // MARK: commands

    private var codeSent = false

    func signIn(server: String, username: String, password: String, code: String? = nil) {
        login = .busy
        codeSent = code != nil
        var cmd: [String: Any] = ["cmd": "login", "server": server, "username": username, "password": password]
        if let code { cmd["code"] = code }
        send(cmd)
    }

    func configure(localDir: String, remoteDir: String) {
        recent = []
        UserDefaults.standard.removeObject(forKey: "recent")
        send(["cmd": "configure", "local_dir": localDir, "remote_dir": remoteDir])
    }

    func syncNow() { send(["cmd": "sync_now"]) }
    func setPaused(_ p: Bool) { send(["cmd": "pause", "paused": p]) }
    func refreshUsage() { send(["cmd": "usage"]) }

    func signOut() {
        send(["cmd": "logout"])
        recent = []
        usage = nil
        login = .idle
        UserDefaults.standard.removeObject(forKey: "recent")
    }

    // MARK: helpers for the UI

    func fullPath(_ a: Activity) -> String { (localDir as NSString).appendingPathComponent(a.path) }

    func reveal(_ a: Activity) {
        let p = fullPath(a)
        if FileManager.default.fileExists(atPath: p) {
            NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: p)])
        }
    }

    func openFolder() {
        guard !localDir.isEmpty else { return }
        NSWorkspace.shared.open(URL(fileURLWithPath: localDir))
    }

    func openWeb() {
        if let u = URL(string: server) { NSWorkspace.shared.open(u) }
    }

    func showLog() {
        guard !logPath.isEmpty else { return }
        NSWorkspace.shared.open(URL(fileURLWithPath: logPath))
    }
}
