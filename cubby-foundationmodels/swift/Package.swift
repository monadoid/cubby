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
    dependencies: [],
    targets: [
        .target(
            name: "FoundationModelsBridge",
            dependencies: [],
            linkerSettings: [
                .linkedFramework("FoundationModels"),
                .linkedFramework("Foundation"),
                .linkedFramework("Speech")
            ]
        )
    ]
)

