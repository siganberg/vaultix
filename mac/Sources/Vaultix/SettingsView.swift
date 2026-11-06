import AppKit
import SwiftUI

struct SettingsView: View {
    @EnvironmentObject var m: AppModel

    var body: some View {
        TabView {
            GeneralTab().tabItem { Label("General", systemImage: "gearshape") }
            AccountTab().tabItem { Label("Account", systemImage: "person.crop.circle") }
            FoldersTab().tabItem { Label("Folders", systemImage: "folder") }
            AboutTab().tabItem { Label("About", systemImage: "info.circle") }
        }
        .padding(.top, 8)
        .frame(width: 520, height: 400)
    }
}

private struct GeneralTab: View {
    @EnvironmentObject var m: AppModel
    @State private var startAtLogin = LoginItem.isEnabled
    @AppStorage("notifications") private var notifications = true

    var body: some View {
        Form {
            Section {
                Toggle("Start Vaultix when I log in", isOn: $startAtLogin)
                    .onChange(of: startAtLogin) { LoginItem.set($0) }
                Toggle("Notify me about conflicts and sync problems", isOn: $notifications)
                    .onChange(of: notifications) { if $0 { Notifier.requestPermission() } }
            }
            Section {
                Toggle("Pause syncing", isOn: Binding(get: { m.paused }, set: { m.setPaused($0) }))
                    .disabled(!m.configured)
                HStack {
                    Text("Check for changes right away")
                    Spacer()
                    Button("Sync Now") { m.syncNow() }.disabled(!m.configured || m.paused)
                }
            } footer: {
                Text("Changes on this Mac sync within seconds; changes from other devices within about 15 seconds.")
                    .font(.caption).foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
    }
}

private struct AccountTab: View {
    @EnvironmentObject var m: AppModel
    @State private var confirm = false

    var body: some View {
        Form {
            if m.configured {
                Section {
                    HStack(spacing: 12) {
                        ZStack {
                            Circle().fill(Color.vxAccent.opacity(0.15))
                            Text(String(m.username.prefix(1)).uppercased())
                                .font(.system(size: 17, weight: .semibold)).foregroundStyle(Color.vxAccent)
                        }
                        .frame(width: 40, height: 40)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(m.username).font(.system(size: 14, weight: .semibold))
                            Text(m.server).font(.system(size: 12)).foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button("Open in Browser") { m.openWeb() }
                    }
                    .padding(.vertical, 4)
                }
                Section {
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Sign out of this Mac")
                            Text("Your files stay in your folder and on the server.")
                                .font(.caption).foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button("Sign Out…", role: .destructive) { confirm = true }
                    }
                } footer: {
                    Text("To revoke this Mac's access remotely, use Settings → Devices in the web app.")
                        .font(.caption).foregroundStyle(.secondary)
                }
            } else {
                Section {
                    HStack {
                        Text("Not signed in")
                        Spacer()
                        Button("Sign In…") { Windows.shared.showSetup() }
                    }
                }
            }
        }
        .formStyle(.grouped)
        .confirmationDialog("Sign out of Vaultix?", isPresented: $confirm) {
            Button("Sign Out", role: .destructive) {
                m.signOut()
                Windows.shared.close("settings")
                Windows.shared.showSetup()
            }
        } message: {
            Text("Syncing stops. Nothing is deleted from this Mac or the server.")
        }
    }
}

private struct FoldersTab: View {
    @EnvironmentObject var m: AppModel
    @State private var local = ""
    @State private var remote = ""

    var changed: Bool {
        local != m.localDir || normalized(remote) != m.remoteDir
    }

    var body: some View {
        Form {
            Section {
                HStack(spacing: 10) {
                    Image(nsImage: NSWorkspace.shared.icon(for: .folder)).resizable().frame(width: 26, height: 26)
                    VStack(alignment: .leading, spacing: 1) {
                        Text("On this Mac").font(.caption).foregroundStyle(.secondary)
                        Text((local as NSString).abbreviatingWithTildeInPath).lineLimit(1).truncationMode(.middle)
                    }
                    Spacer()
                    Button("Show") { NSWorkspace.shared.open(URL(fileURLWithPath: local)) }
                    Button("Change…", action: choose)
                }
                HStack {
                    Text("Folder on the server")
                    Spacer()
                    TextField("/", text: $remote).multilineTextAlignment(.trailing).frame(width: 200)
                }
            } footer: {
                Text("Changing folders starts a fresh sync. Nothing is deleted: files are merged, and anything edited in both places keeps both copies.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            Section {
                HStack {
                    Spacer()
                    Button("Revert") { load() }.disabled(!changed)
                    Button("Apply") { m.configure(localDir: local, remoteDir: remote) }
                        .keyboardShortcut(.defaultAction)
                        .disabled(!changed || !m.configured)
                }
            }
        }
        .formStyle(.grouped)
        .onAppear(perform: load)
        .onChange(of: m.localDir) { _ in load() }
        .onChange(of: m.remoteDir) { _ in load() }
    }

    private func normalized(_ s: String) -> String {
        s.trimmingCharacters(in: CharacterSet(charactersIn: "/ "))
    }

    private func load() {
        local = m.localDir
        remote = "/" + m.remoteDir
    }

    private func choose() {
        let p = NSOpenPanel()
        p.canChooseDirectories = true
        p.canChooseFiles = false
        p.canCreateDirectories = true
        p.prompt = "Use This Folder"
        p.directoryURL = URL(fileURLWithPath: local)
        NSApp.activate(ignoringOtherApps: true)
        if p.runModal() == .OK, let u = p.url { local = u.path }
    }
}

private struct AboutTab: View {
    @EnvironmentObject var m: AppModel
    var body: some View {
        VStack(spacing: 10) {
            Spacer()
            LogoView(size: 64)
            Text("Vaultix").font(.system(size: 20, weight: .semibold)).padding(.top, 6)
            Text("Version \(Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? m.version)")
                .font(.system(size: 12)).foregroundStyle(.secondary)
            Text("Your files, on your own server.")
                .font(.system(size: 12.5)).foregroundStyle(.secondary)
            HStack {
                Button("Show Log") { m.showLog() }
                Button("Open Web App") { m.openWeb() }.disabled(!m.configured)
            }
            .padding(.top, 8)
            Spacer()
        }
        .frame(maxWidth: .infinity)
    }
}
