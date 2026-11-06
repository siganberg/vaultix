import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct PopoverView: View {
    @EnvironmentObject var m: AppModel

    var body: some View {
        VStack(spacing: 0) {
            header
            if let err = m.helperError {
                Banner(symbol: "exclamationmark.octagon.fill", color: .red, text: err)
            }
            if m.configured, let t = m.transfer, m.phase == "syncing" {
                TransferCard(t: t)
                    .padding(.horizontal, 14)
                    .padding(.bottom, 12)
                    .transition(.opacity.combined(with: .move(edge: .top)))
            }
            Divider()
            content
            Divider()
            footer
        }
        .frame(width: 360)
        .animation(.easeOut(duration: 0.2), value: m.transfer?.path)
        .onAppear { if m.configured { m.refreshUsage() } }
    }

    // MARK: header

    private var header: some View {
        HStack(spacing: 11) {
            LogoView(size: 34)
            VStack(alignment: .leading, spacing: 3) {
                Text("Vaultix").font(.system(size: 14, weight: .semibold))
                StatusLine()
            }
            Spacer(minLength: 8)
            if m.configured {
                Button {
                    m.setPaused(!m.paused)
                } label: {
                    Image(systemName: m.paused ? "play.fill" : "pause.fill")
                        .font(.system(size: 11, weight: .semibold))
                        .frame(width: 26, height: 26)
                        .background(Circle().fill(Color.primary.opacity(0.07)))
                }
                .buttonStyle(.plain)
                .help(m.paused ? "Resume syncing" : "Pause syncing")
            }
            Menu {
                if m.configured {
                    Button("Sync Now") { m.syncNow() }
                    Button(m.paused ? "Resume Syncing" : "Pause Syncing") { m.setPaused(!m.paused) }
                    Divider()
                    Button("Open in Browser") { m.openWeb() }
                }
                Button("Settings…") { Windows.shared.showSettings() }
                Button("Show Log") { m.showLog() }
                Divider()
                Button("Quit Vaultix") { m.quit() }
            } label: {
                Image(systemName: "ellipsis")
                    .font(.system(size: 13, weight: .semibold))
                    .frame(width: 26, height: 26)
                    .background(Circle().fill(Color.primary.opacity(0.07)))
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
        }
        .padding(14)
    }

    // MARK: content

    @ViewBuilder private var content: some View {
        if !m.configured {
            VStack(spacing: 12) {
                Image(systemName: "icloud.and.arrow.up")
                    .font(.system(size: 30, weight: .light))
                    .foregroundStyle(Color.vxAccent)
                Text("Sign in to start syncing").font(.system(size: 14, weight: .semibold))
                Text("Connect this Mac to your Vaultix drive.")
                    .font(.system(size: 12)).foregroundStyle(.secondary)
                Button("Sign In…") { Windows.shared.showSetup() }
                    .buttonStyle(AccentButtonStyle())
                    .padding(.top, 2)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 30)
        } else if m.phase == "signedout" {
            VStack(spacing: 10) {
                Image(systemName: "person.crop.circle.badge.exclamationmark")
                    .font(.system(size: 28, weight: .light)).foregroundStyle(.orange)
                Text("Your session ended").font(.system(size: 14, weight: .semibold))
                Text("The app password may have been revoked.").font(.system(size: 12)).foregroundStyle(.secondary)
                Button("Sign In Again…") { m.signOut(); Windows.shared.showSetup() }
                    .buttonStyle(AccentButtonStyle())
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 26)
        } else if m.recent.isEmpty {
            VStack(spacing: 8) {
                Image(systemName: "checkmark.circle")
                    .font(.system(size: 28, weight: .light)).foregroundStyle(.green)
                Text(m.phase == "syncing" ? "Syncing your files…" : "Everything's up to date")
                    .font(.system(size: 13, weight: .medium))
                Text("Files you add to your Vaultix folder appear here.")
                    .font(.system(size: 11.5)).foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 30)
        } else {
            VStack(alignment: .leading, spacing: 0) {
                Text("RECENT")
                    .font(.system(size: 10.5, weight: .semibold))
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 16)
                    .padding(.top, 10)
                    .padding(.bottom, 4)
                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(m.recent.prefix(30)) { a in
                            ActivityRow(a: a)
                        }
                    }
                    .padding(.horizontal, 6)
                    .padding(.bottom, 6)
                }
                .frame(maxHeight: 300)
            }
        }
    }

    // MARK: footer

    private var footer: some View {
        HStack(spacing: 6) {
            if m.configured {
                FooterButton(symbol: "folder", title: "Folder") { m.openFolder() }
                FooterButton(symbol: "safari", title: "Web") { m.openWeb() }
            } else {
                FooterButton(symbol: "power", title: "Quit Vaultix") { m.quit() }
            }
            Spacer()
            if let u = m.usage {
                VStack(alignment: .trailing, spacing: 3) {
                    Text("\(Fmt.bytes(u.used)) in Vaultix")
                        .font(.system(size: 11, weight: .medium))
                    Text("\(Fmt.bytes(u.free)) free on server")
                        .font(.system(size: 10.5)).foregroundStyle(.secondary)
                }
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 9)
    }
}

// MARK: - Pieces

struct StatusLine: View {
    @EnvironmentObject var m: AppModel
    var body: some View {
        HStack(spacing: 6) {
            if m.phase == "syncing" {
                ProgressView().controlSize(.mini).scaleEffect(0.8).frame(width: 9, height: 9)
            } else {
                Circle().fill(color).frame(width: 7, height: 7)
            }
            TimelineView(.periodic(from: .now, by: 30)) { ctx in
                Text(text(now: ctx.date))
                    .font(.system(size: 11.5))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
        }
    }

    private var color: Color {
        if !m.configured { return .gray }
        switch m.phase {
        case "idle": return .green
        case "paused": return .gray
        case "offline": return .orange
        case "error", "signedout": return .red
        default: return .vxAccent
        }
    }

    private func text(now: Date) -> String {
        if !m.configured { return "Not signed in" }
        switch m.phase {
        case "idle":
            if let d = m.lastOk { return "Up to date · \(Fmt.ago(d, now: now))" }
            return "Up to date"
        case "syncing":
            if let t = m.transfer, t.total > 0 { return "Syncing \(t.index) of \(t.total)…" }
            return "Checking for changes…"
        default:
            return m.statusLine
        }
    }
}

struct TransferCard: View {
    let t: Transfer
    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            HStack(spacing: 8) {
                Image(systemName: t.op == "Uploading" ? "arrow.up" : t.op == "Downloading" ? "arrow.down" : "arrow.triangle.2.circlepath")
                    .font(.system(size: 11, weight: .bold))
                    .foregroundStyle(Color.vxAccent)
                    .frame(width: 22, height: 22)
                    .background(RoundedRectangle(cornerRadius: 6).fill(Color.vxAccent.opacity(0.14)))
                VStack(alignment: .leading, spacing: 1) {
                    Text(t.name).font(.system(size: 12, weight: .medium)).lineLimit(1).truncationMode(.middle)
                    Text(detail).font(.system(size: 10.5)).foregroundStyle(.secondary)
                }
            }
            BrandBar(fraction: t.fraction)
        }
        .padding(11)
        .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Color.primary.opacity(0.045)))
    }

    private var detail: String {
        var s = t.op
        if t.size > 0 { s += " · \(Fmt.bytes(t.bytes)) of \(Fmt.bytes(t.size))" }
        if t.total > 1 { s += " · \(t.index) of \(t.total)" }
        return s
    }
}

struct ActivityRow: View {
    @EnvironmentObject var m: AppModel
    let a: Activity
    @State private var hover = false

    var body: some View {
        Button { m.reveal(a) } label: {
            HStack(spacing: 10) {
                ZStack(alignment: .bottomTrailing) {
                    FileIcon(path: m.fullPath(a), name: a.name, isDir: a.isDir)
                        .frame(width: 30, height: 30)
                        .opacity(a.isDelete ? 0.45 : 1)
                    Image(systemName: a.symbol)
                        .font(.system(size: 11))
                        .foregroundStyle(.white, badgeColor)
                        .background(Circle().fill(Color(nsColor: .windowBackgroundColor)).padding(-1))
                        .offset(x: 3, y: 3)
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text(a.name)
                        .font(.system(size: 12.5, weight: .medium))
                        .lineLimit(1).truncationMode(.middle)
                        .strikethrough(a.isDelete, color: .secondary)
                    TimelineView(.periodic(from: .now, by: 30)) { ctx in
                        Text(subtitle(ctx.date))
                            .font(.system(size: 11)).foregroundStyle(.secondary)
                            .lineLimit(1).truncationMode(.middle)
                    }
                }
                Spacer(minLength: 4)
                if hover && !a.isDelete {
                    Image(systemName: "magnifyingglass")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(.secondary)
                        .help("Show in Finder")
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
            .contentShape(Rectangle())
            .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(hover ? Color.primary.opacity(0.06) : .clear))
        }
        .buttonStyle(.plain)
        .onHover { hover = $0 }
    }

    private var badgeColor: Color {
        switch a.op {
        case "uploaded": return .vxAccent
        case "downloaded": return .vxAccent2
        case "conflict": return .orange
        default: return .gray
        }
    }

    private func subtitle(_ now: Date) -> String {
        var s = "\(a.verb) · \(Fmt.ago(a.at, now: now))"
        if !a.folder.isEmpty { s += " · \(a.folder)" }
        return s
    }
}

/// Finder's icon for the file when it exists locally, otherwise one by extension.
struct FileIcon: View {
    let path: String
    let name: String
    let isDir: Bool
    var body: some View {
        Image(nsImage: icon).resizable().interpolation(.high)
    }
    private var icon: NSImage {
        if FileManager.default.fileExists(atPath: path) { return NSWorkspace.shared.icon(forFile: path) }
        if isDir { return NSWorkspace.shared.icon(for: .folder) }
        let ext = (name as NSString).pathExtension
        if let t = UTType(filenameExtension: ext) { return NSWorkspace.shared.icon(for: t) }
        return NSWorkspace.shared.icon(for: .data)
    }
}

struct FooterButton: View {
    let symbol: String
    let title: String
    let action: () -> Void
    @State private var hover = false
    @Environment(\.isEnabled) private var enabled
    var body: some View {
        Button(action: action) {
            HStack(spacing: 5) {
                Image(systemName: symbol).font(.system(size: 11.5))
                Text(title).font(.system(size: 12, weight: .medium))
            }
            .padding(.horizontal, 9)
            .padding(.vertical, 6)
            .background(RoundedRectangle(cornerRadius: 7, style: .continuous).fill(hover && enabled ? Color.primary.opacity(0.07) : .clear))
            .opacity(enabled ? 1 : 0.4)
        }
        .buttonStyle(.plain)
        .onHover { hover = $0 }
    }
}

/// Slim progress bar in the brand gradient; animates indeterminately when size is unknown.
struct BrandBar: View {
    var fraction: Double?
    @State private var phase: CGFloat = -0.3
    var body: some View {
        GeometryReader { g in
            ZStack(alignment: .leading) {
                Capsule().fill(Color.primary.opacity(0.08))
                let fill = LinearGradient(colors: [Color(red: 0.486, green: 0.424, blue: 1), .vxAccent2], startPoint: .leading, endPoint: .trailing)
                if let f = fraction {
                    Capsule().fill(fill).frame(width: max(6, g.size.width * f))
                        .animation(.easeOut(duration: 0.25), value: f)
                } else {
                    Capsule().fill(fill).frame(width: g.size.width * 0.3)
                        .offset(x: g.size.width * phase)
                        .onAppear { withAnimation(.easeInOut(duration: 1.1).repeatForever(autoreverses: true)) { phase = 0.7 } }
                }
            }
            .clipShape(Capsule())
        }
        .frame(height: 5)
    }
}

struct Banner: View {
    let symbol: String
    let color: Color
    let text: String
    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: symbol).foregroundStyle(color)
            Text(text).font(.system(size: 11.5)).fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
        }
        .padding(10)
        .background(RoundedRectangle(cornerRadius: 8).fill(color.opacity(0.1)))
        .padding(.horizontal, 14)
        .padding(.bottom, 10)
    }
}

struct AccentButtonStyle: ButtonStyle {
    var wide = false
    @Environment(\.isEnabled) private var enabled
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(.white)
            .frame(maxWidth: wide ? .infinity : nil)
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
            .background(
                RoundedRectangle(cornerRadius: 9, style: .continuous)
                    .fill(Color.vxAccent.opacity(configuration.isPressed ? 0.8 : 1))
            )
            .opacity(enabled ? 1 : 0.5)
            .scaleEffect(configuration.isPressed ? 0.98 : 1)
    }
}
