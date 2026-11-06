import AppKit
import SwiftUI

struct SetupView: View {
    @EnvironmentObject var m: AppModel

    enum Step { case signIn, code, folders, done }
    @State private var step: Step = .signIn

    @State private var server = ""
    @State private var username = ""
    @State private var password = ""
    @State private var code = ""
    @State private var localDir = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Vaultix").path
    @State private var remoteDir = "/"
    @State private var startAtLogin = true
    @FocusState private var focus: Field?
    enum Field { case server, username, password, code, remote }

    var body: some View {
        ZStack {
            Color(nsColor: .windowBackgroundColor).ignoresSafeArea()
            VStack(spacing: 0) {
                switch step {
                case .signIn: signIn
                case .code: codeStep
                case .folders: folders
                case .done: done
                }
            }
            .padding(.horizontal, 40)
            .padding(.top, 34)
            .padding(.bottom, 28)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            .animation(.easeInOut(duration: 0.2), value: step)
        }
        .frame(width: 440, height: 560)
        .onAppear {
            server = m.server.isEmpty ? (UserDefaults.standard.string(forKey: "lastServer") ?? "") : m.server
            username = m.username.isEmpty ? (UserDefaults.standard.string(forKey: "lastUser") ?? "") : m.username
            m.login = .idle
            DispatchQueue.main.async { focus = server.isEmpty ? .server : username.isEmpty ? .username : .password }
        }
        .onChange(of: m.login) { s in
            switch s {
            case .success:
                UserDefaults.standard.set(server, forKey: "lastServer")
                UserDefaults.standard.set(username, forKey: "lastUser")
                password = ""
                step = .folders
            case .needCode:
                code = ""
                step = .code
                DispatchQueue.main.async { focus = .code }
            case .codeRejected:
                code = ""
                focus = .code
            default: break
            }
        }
    }

    // MARK: steps

    private var signIn: some View {
        VStack(spacing: 0) {
            LogoView(size: 58).padding(.bottom, 18)
            Text("Sign in to Vaultix").font(.system(size: 21, weight: .semibold))
            Text("Connect this Mac to your drive.")
                .font(.system(size: 13)).foregroundStyle(.secondary)
                .padding(.top, 4).padding(.bottom, 24)

            VStack(spacing: 14) {
                LabeledField(label: "Server", focused: focus == .server) {
                    TextField("vaultix.example.com", text: $server)
                        .focused($focus, equals: .server)
                        .onSubmit { focus = .username }
                }
                LabeledField(label: "Username", focused: focus == .username) {
                    TextField("", text: $username)
                        .focused($focus, equals: .username)
                        .onSubmit { focus = .password }
                }
                LabeledField(label: "Password", hint: "An app password from Settings works too, and skips 2FA.", focused: focus == .password) {
                    SecureField("", text: $password)
                        .focused($focus, equals: .password)
                        .onSubmit(submit)
                }
            }

            errorView.padding(.top, 12)
            Spacer(minLength: 12)
            Button(action: submit) {
                HStack(spacing: 8) {
                    if m.login == .busy { ProgressView().controlSize(.small).tint(.white) }
                    Text(m.login == .busy ? "Signing in…" : "Sign In")
                }
            }
            .buttonStyle(AccentButtonStyle(wide: true))
            .keyboardShortcut(.defaultAction)
            .disabled(m.login == .busy || server.isEmpty || username.isEmpty || password.isEmpty)
        }
    }

    private var codeStep: some View {
        VStack(spacing: 0) {
            Image(systemName: "lock.shield.fill")
                .font(.system(size: 40))
                .foregroundStyle(Color.vxAccent)
                .padding(.bottom, 18)
                .padding(.top, 12)
            Text("Two-factor authentication").font(.system(size: 21, weight: .semibold))
            Text("Enter the 6-digit code from your authenticator app,\nor one of your recovery codes.")
                .font(.system(size: 13)).foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.top, 6).padding(.bottom, 26)

            TextField("000000", text: $code)
                .textFieldStyle(.plain)
                .font(.system(size: 30, weight: .semibold, design: .monospaced))
                .multilineTextAlignment(.center)
                .padding(.vertical, 12)
                .background(FieldBackground(focused: focus == .code))
                .focused($focus, equals: .code)
                .onSubmit(submitCode)
                .onChange(of: code) { c in
                    if c.count == 6, c.allSatisfy(\.isNumber), m.login != .busy { submitCode() }
                }

            Group {
                if m.login == .codeRejected {
                    Label("That code didn't work. Try the next one.", systemImage: "exclamationmark.circle.fill")
                        .foregroundStyle(.red)
                } else {
                    errorView
                }
            }
            .font(.system(size: 12))
            .padding(.top, 12)

            Spacer(minLength: 12)
            HStack(spacing: 10) {
                Button("Back") { m.login = .idle; step = .signIn }
                    .buttonStyle(.bordered).controlSize(.large)
                Button(action: submitCode) {
                    HStack(spacing: 8) {
                        if m.login == .busy { ProgressView().controlSize(.small).tint(.white) }
                        Text("Verify")
                    }
                }
                .buttonStyle(AccentButtonStyle(wide: true))
                .keyboardShortcut(.defaultAction)
                .disabled(m.login == .busy || code.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
    }

    private var folders: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack { Spacer(); Image(systemName: "folder.badge.gearshape").font(.system(size: 40)).foregroundStyle(Color.vxAccent); Spacer() }
                .padding(.bottom, 16).padding(.top, 8)
            Text("Choose what to sync").font(.system(size: 21, weight: .semibold)).frame(maxWidth: .infinity)
            Text("Signed in as \(username). Pick where your files live on this Mac.")
                .font(.system(size: 13)).foregroundStyle(.secondary)
                .frame(maxWidth: .infinity).multilineTextAlignment(.center)
                .padding(.top, 4).padding(.bottom, 24)

            Text("On this Mac").font(.system(size: 12, weight: .medium)).foregroundStyle(.secondary).padding(.bottom, 6)
            HStack(spacing: 10) {
                Image(nsImage: NSWorkspace.shared.icon(for: .folder)).resizable().frame(width: 28, height: 28)
                VStack(alignment: .leading, spacing: 1) {
                    Text((localDir as NSString).lastPathComponent).font(.system(size: 13, weight: .medium))
                    Text((localDir as NSString).abbreviatingWithTildeInPath).font(.system(size: 11)).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                }
                Spacer()
                Button("Change…", action: chooseFolder).controlSize(.regular)
            }
            .padding(10)
            .background(FieldBackground(focused: false))
            .padding(.bottom, 16)

            LabeledField(label: "Folder on the server", hint: "“/” syncs your whole drive. Use e.g. /Projects to sync just one folder.", focused: focus == .remote) {
                TextField("/", text: $remoteDir).focused($focus, equals: .remote)
            }

            Toggle("Start Vaultix when I log in", isOn: $startAtLogin)
                .toggleStyle(.checkbox)
                .font(.system(size: 12.5))
                .padding(.top, 16)

            Spacer(minLength: 12)
            Button("Start Syncing", action: finish)
                .buttonStyle(AccentButtonStyle(wide: true))
                .keyboardShortcut(.defaultAction)
        }
    }

    private var done: some View {
        VStack(spacing: 0) {
            Spacer()
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 56))
                .foregroundStyle(.green)
                .padding(.bottom, 18)
            Text("You're all set").font(.system(size: 21, weight: .semibold))
            Text("Vaultix lives in your menu bar. Click its icon any time to see what's syncing.")
                .font(.system(size: 13)).foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.top, 6)
            HStack(spacing: 8) {
                Image(nsImage: MenuBarIcon.image(dimmed: false)).renderingMode(.template)
                Image(systemName: "arrow.up.right").font(.system(size: 11, weight: .semibold)).foregroundStyle(.secondary)
            }
            .padding(.top, 18)
            Spacer()
            HStack(spacing: 10) {
                Button("Open Folder") { m.openFolder() }.buttonStyle(.bordered).controlSize(.large)
                Button("Done") { Windows.shared.close("setup") }
                    .buttonStyle(AccentButtonStyle(wide: true))
                    .keyboardShortcut(.defaultAction)
            }
        }
    }

    @ViewBuilder private var errorView: some View {
        if case .failed(let msg, let lan) = m.login {
            VStack(spacing: 8) {
                Label(msg, systemImage: "exclamationmark.circle.fill")
                    .font(.system(size: 12))
                    .foregroundStyle(.red)
                    .fixedSize(horizontal: false, vertical: true)
                if lan {
                    Text("Allow Vaultix under Local Network, then try again.")
                        .font(.system(size: 11.5)).foregroundStyle(.secondary)
                    Button("Open Privacy Settings") {
                        NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_LocalNetwork")!)
                    }
                    .controlSize(.small)
                }
            }
            .frame(maxWidth: .infinity)
        }
    }

    // MARK: actions

    private func submit() {
        guard !server.isEmpty, !username.isEmpty, !password.isEmpty, m.login != .busy else { return }
        m.signIn(server: server, username: username, password: password)
    }

    private func submitCode() {
        let c = code.trimmingCharacters(in: .whitespaces)
        guard !c.isEmpty, m.login != .busy else { return }
        m.signIn(server: server, username: username, password: password, code: c)
    }

    private func chooseFolder() {
        let p = NSOpenPanel()
        p.canChooseDirectories = true
        p.canChooseFiles = false
        p.canCreateDirectories = true
        p.prompt = "Use This Folder"
        p.message = "Choose the folder to sync with Vaultix"
        p.directoryURL = URL(fileURLWithPath: (localDir as NSString).deletingLastPathComponent)
        NSApp.activate(ignoringOtherApps: true)
        if p.runModal() == .OK, let u = p.url { localDir = u.path }
    }

    private func finish() {
        m.configure(localDir: localDir, remoteDir: remoteDir)
        LoginItem.set(startAtLogin)
        Notifier.requestPermission()
        step = .done
    }
}

// MARK: - Field chrome

struct LabeledField<Content: View>: View {
    let label: String
    var hint: String? = nil
    var focused: Bool = false
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label).font(.system(size: 12, weight: .medium)).foregroundStyle(.secondary)
            content
                .textFieldStyle(.plain)
                .font(.system(size: 13.5))
                .padding(.horizontal, 11)
                .padding(.vertical, 9)
                .background(FieldBackground(focused: focused))
            if let hint {
                Text(hint).font(.system(size: 11)).foregroundStyle(.tertiary).fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

struct FieldBackground: View {
    var focused: Bool
    var body: some View {
        RoundedRectangle(cornerRadius: 9, style: .continuous)
            .fill(Color(nsColor: .textBackgroundColor))
            .overlay(
                RoundedRectangle(cornerRadius: 9, style: .continuous)
                    .strokeBorder(focused ? Color.vxAccent : Color.primary.opacity(0.14), lineWidth: focused ? 1.5 : 1)
            )
            .shadow(color: focused ? Color.vxAccent.opacity(0.18) : .clear, radius: 4)
    }
}
