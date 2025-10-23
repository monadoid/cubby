// swift-tools-version: 6.0
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "FoundationModelsBridge",
    platforms: [
        .macOS("26.0")
    ],
    products: [
        .library(name: "FoundationModelsBridge", type: .static, targets: ["FoundationModelsBridge"])
    ],
    dependencies: [
        .package(url: "https://github.com/Brendonovich/swift-rs", from: "1.0.5")
    ],
    targets: [
        .target(
            name: "FoundationModelsBridge",
            dependencies: [
                .product(name: "SwiftRs", package: "swift-rs")
            ],
            linkerSettings: [
                .linkedFramework("FoundationModels"),
                .linkedFramework("Foundation"),
                .linkedFramework("Speech"),
                .linkedFramework("AVFoundation")
            ]
        )
    ]
)
