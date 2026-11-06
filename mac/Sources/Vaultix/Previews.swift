import AppKit
import SwiftUI

/// Developer aid: `Vaultix --render-previews <dir>` renders the main screens with
/// sample data to PNGs (light + dark) and exits. Never starts the sync helper.
@MainActor
enum Previews {
    static func run(dir: String) {
        try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
        let m = AppModel.shared
        m.configured = true
        m.username = "francis"
        m.server = "https://vaultix.marasigan.net"
        m.localDir = NSHomeDirectory() + "/Vaultix"
        m.remoteDir = ""
        m.usage = Usage(used: 38_440_539_000, free: 567_722_000_384, total: 1_995_218_165_760)
        let now = Date()
        m.recent = [
            Activity(op: "uploaded", path: "CNC Projects/Enclosure/lid.stl", isDir: false, at: now.addingTimeInterval(-40)),
            Activity(op: "downloaded", path: "Photos/sunset.png", isDir: false, at: now.addingTimeInterval(-300)),
            Activity(op: "conflict", path: "CNC Projects/notes (conflict MacServer 2026-11-06 0912).md", isDir: false, at: now.addingTimeInterval(-900), original: "CNC Projects/notes.md"),
            Activity(op: "deleted_remote", path: "Old/drafts.txt", isDir: false, at: now.addingTimeInterval(-3600)),
            Activity(op: "downloaded", path: "Report.pdf", isDir: false, at: now.addingTimeInterval(-7200)),
        ]

        for dark in [false, true] {
            let suffix = dark ? "-dark" : ""
            m.phase = "syncing"
            m.transfer = Transfer(op: "Uploading", path: "CNC Projects/Enclosure/lid.stl", index: 3, total: 12, bytes: 18_400_000, size: 42_000_000)
            render(PopoverView().environmentObject(m), "popover-syncing\(suffix)", dir, dark)
            m.phase = "idle"
            m.transfer = nil
            m.lastOk = now.addingTimeInterval(-120)
            render(PopoverView().environmentObject(m), "popover-idle\(suffix)", dir, dark)
            render(SetupView().environmentObject(m), "setup\(suffix)", dir, dark)
            render(SettingsView().environmentObject(m), "settings\(suffix)", dir, dark)
        }
        m.configured = false
        m.recent = []
        m.usage = nil
        render(PopoverView().environmentObject(m), "popover-signedout", dir, false)
    }

    private static func render<V: View>(_ view: V, _ name: String, _ dir: String, _ dark: Bool) {
        let host = NSHostingView(rootView: view.background(Color(nsColor: .windowBackgroundColor)))
        host.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        host.frame.size = host.fittingSize
        let w = NSWindow(contentRect: host.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        w.appearance = host.appearance
        w.contentView = host
        w.orderFrontRegardless()
        w.setFrameOrigin(NSPoint(x: -10000, y: -10000))
        RunLoop.main.run(until: Date().addingTimeInterval(0.6))
        host.layoutSubtreeIfNeeded()
        guard let rep = host.bitmapImageRepForCachingDisplay(in: host.bounds) else { return }
        host.cacheDisplay(in: host.bounds, to: rep)
        if let png = rep.representation(using: .png, properties: [:]) {
            try? png.write(to: URL(fileURLWithPath: dir).appendingPathComponent("\(name).png"))
        }
        w.orderOut(nil)
    }
}
