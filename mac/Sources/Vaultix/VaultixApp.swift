import AppKit
import SwiftUI

@main
struct VaultixApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var delegate
    @StateObject private var model = AppModel.shared

    var body: some Scene {
        MenuBarExtra {
            PopoverView().environmentObject(model)
        } label: {
            Image(nsImage: MenuBarIcon.image(dimmed: ["paused", "offline", "error", "signedout"].contains(model.phase) || !model.configured))
                .accessibilityLabel("Vaultix")
        }
        .menuBarExtraStyle(.window)
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        let args = CommandLine.arguments
        if let i = args.firstIndex(of: "--render-previews"), i + 1 < args.count {
            Task { @MainActor in
                Previews.run(dir: args[i + 1])
                exit(0)
            }
            return
        }
        // Single instance.
        let mine = NSRunningApplication.current
        if let other = NSRunningApplication.runningApplications(withBundleIdentifier: Bundle.main.bundleIdentifier ?? "")
            .first(where: { $0.processIdentifier != mine.processIdentifier }) {
            other.activate()
            NSApp.terminate(nil)
            return
        }
        NSApp.setActivationPolicy(.accessory)
        LoginItem.migrate()
        Task { @MainActor in AppModel.shared.start() }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }
}
