// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "Vaultix",
    platforms: [.macOS(.v13)],
    targets: [
        .executableTarget(name: "Vaultix", path: "Sources/Vaultix")
    ]
)
