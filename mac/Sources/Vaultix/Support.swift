import AppKit
import SwiftUI
import UserNotifications

// MARK: - Brand

extension Color {
    static let vxAccent = Color(red: 0.416, green: 0.361, blue: 1.0)      // #6a5cff
    static let vxAccent2 = Color(red: 0.227, green: 0.545, blue: 1.0)     // #3a8bff
}

/// The Vaultix mark: a "V" on a violet→blue rounded square.
struct LogoView: View {
    var size: CGFloat = 28
    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: size * 0.24, style: .continuous)
                .fill(LinearGradient(colors: [Color(red: 0.486, green: 0.424, blue: 1), .vxAccent2], startPoint: .topLeading, endPoint: .bottomTrailing))
            VShape()
                .stroke(.white, style: StrokeStyle(lineWidth: size * 0.095, lineCap: .round, lineJoin: .round))
                .padding(size * 0.28)
        }
        .frame(width: size, height: size)
        .shadow(color: .vxAccent.opacity(0.25), radius: size * 0.12, y: size * 0.04)
    }
}

struct VShape: Shape {
    func path(in r: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: r.minX, y: r.minY + r.height * 0.08))
        p.addLine(to: CGPoint(x: r.midX, y: r.maxY - r.height * 0.04))
        p.addLine(to: CGPoint(x: r.maxX, y: r.minY + r.height * 0.08))
        return p
    }
}

// MARK: - Menu bar icon

enum MenuBarIcon {
    /// Template image so macOS tints it for light/dark menu bars.
    static func image(dimmed: Bool) -> NSImage {
        let size = NSSize(width: 18, height: 18)
        let img = NSImage(size: size, flipped: true) { _ in
            let alpha: CGFloat = dimmed ? 0.45 : 1
            NSColor.black.withAlphaComponent(alpha).setStroke()
            let box = NSBezierPath(roundedRect: NSRect(x: 1.75, y: 1.75, width: 14.5, height: 14.5), xRadius: 4.2, yRadius: 4.2)
            box.lineWidth = 1.5
            box.stroke()
            let v = NSBezierPath()
            v.move(to: NSPoint(x: 5.6, y: 5.8))
            v.line(to: NSPoint(x: 9, y: 12.4))
            v.line(to: NSPoint(x: 12.4, y: 5.8))
            v.lineWidth = 1.9
            v.lineCapStyle = .round
            v.lineJoinStyle = .round
            v.stroke()
            return true
        }
        img.isTemplate = true
        return img
    }
}

// MARK: - Formatting

enum Fmt {
    static func bytes(_ n: Int64) -> String {
        ByteCountFormatter.string(fromByteCount: n, countStyle: .file)
    }
    static func ago(_ d: Date, now: Date = Date()) -> String {
        if now.timeIntervalSince(d) < 45 { return "just now" }
        let f = RelativeDateTimeFormatter()
        f.unitsStyle = .short
        return f.localizedString(for: d, relativeTo: now)
    }
}

// MARK: - Windows

/// Plain AppKit windows hosting SwiftUI views; reliable for a menu-bar-only (LSUIElement) app.
@MainActor
final class Windows: NSObject, NSWindowDelegate {
    static let shared = Windows()
    private var windows: [String: NSWindow] = [:]

    func showSetup() {
        show(id: "setup", title: "Sign in to Vaultix", size: NSSize(width: 440, height: 560)) {
            SetupView().environmentObject(AppModel.shared)
        }
    }

    func showSettings() {
        show(id: "settings", title: "Vaultix Settings", size: NSSize(width: 520, height: 400)) {
            SettingsView().environmentObject(AppModel.shared)
        }
    }

    func close(_ id: String) {
        windows[id]?.close()
    }

    private func show<V: View>(id: String, title: String, size: NSSize, view: () -> V) {
        if let w = windows[id] {
            NSApp.activate(ignoringOtherApps: true)
            w.makeKeyAndOrderFront(nil)
            return
        }
        let w = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled, .closable, .fullSizeContentView], backing: .buffered, defer: false)
        w.title = title
        w.titlebarAppearsTransparent = true
        w.isReleasedWhenClosed = false
        w.contentView = NSHostingView(rootView: view())
        w.center()
        w.delegate = self
        windows[id] = w
        NSApp.activate(ignoringOtherApps: true)
        w.makeKeyAndOrderFront(nil)
    }

    func windowWillClose(_ n: Notification) {
        guard let w = n.object as? NSWindow else { return }
        windows = windows.filter { $0.value !== w }
    }
}

// MARK: - Notifications

enum Notifier {
    static var enabled: Bool {
        UserDefaults.standard.object(forKey: "notifications") as? Bool ?? true
    }

    static func requestPermission() {
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { _, _ in }
    }

    static func post(title: String, body: String) {
        guard enabled else { return }
        let c = UNMutableNotificationContent()
        c.title = title
        c.body = body
        UNUserNotificationCenter.current().add(UNNotificationRequest(identifier: UUID().uuidString, content: c, trigger: nil))
    }
}

// MARK: - Start at login (LaunchAgent → `open -a Vaultix.app`)

enum LoginItem {
    static let label = "app.vaultix.sync"
    static var plistURL: URL {
        FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/LaunchAgents/\(label).plist")
    }

    static var isEnabled: Bool { FileManager.default.fileExists(atPath: plistURL.path) }

    static func set(_ on: Bool) {
        if !on {
            try? FileManager.default.removeItem(at: plistURL)
            return
        }
        let plist: [String: Any] = [
            "Label": label,
            // Launch through LaunchServices so macOS attributes permissions to Vaultix.app.
            "ProgramArguments": ["/usr/bin/open", "-g", "-a", Bundle.main.bundlePath],
            "RunAtLoad": true,
            "ProcessType": "Interactive",
        ]
        try? FileManager.default.createDirectory(at: plistURL.deletingLastPathComponent(), withIntermediateDirectories: true)
        if let data = try? PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0) {
            try? data.write(to: plistURL)
        }
    }

    /// Older builds pointed the agent at the helper binary; keep it pointing at this app.
    static func migrate() {
        if isEnabled { set(true) }
    }
}
