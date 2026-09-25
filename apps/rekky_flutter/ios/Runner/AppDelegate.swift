import Flutter
import UIKit

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {
  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  func didInitializeImplicitFlutterEngine(_ engineBridge: FlutterImplicitEngineBridge) {
    GeneratedPluginRegistrant.register(with: engineBridge.pluginRegistry)
    let channel = FlutterMethodChannel(
      name: "app.rekky/voice_storage",
      binaryMessenger: engineBridge.applicationRegistrar.messenger()
    )
    channel.setMethodCallHandler { call, result in
      do {
        let directory = try Self.voiceDirectory()
        switch call.method {
        case "directory":
          result(directory.path)
        case "protectFile":
          guard let arguments = call.arguments as? [String: Any],
                let path = arguments["path"] as? String else {
            throw VoiceStorageError.invalidPath
          }
          var file = URL(fileURLWithPath: path).resolvingSymlinksInPath()
          guard file.deletingLastPathComponent() == directory.resolvingSymlinksInPath(),
                FileManager.default.fileExists(atPath: file.path) else {
            throw VoiceStorageError.invalidPath
          }
          var values = URLResourceValues()
          values.isExcludedFromBackup = true
          try file.setResourceValues(values)
          try FileManager.default.setAttributes(
            [.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication],
            ofItemAtPath: file.path
          )
          result(nil)
        default:
          result(FlutterMethodNotImplemented)
        }
      } catch {
        result(FlutterError(code: "storage", message: String(describing: error), details: nil))
      }
    }
  }

  private enum VoiceStorageError: Error { case invalidPath }

  private static func voiceDirectory() throws -> URL {
    let manager = FileManager.default
    var directory = try manager.url(
      for: .applicationSupportDirectory,
      in: .userDomainMask,
      appropriateFor: nil,
      create: true
    ).appendingPathComponent("rekky_voice_drafts_v2", isDirectory: true)
    try manager.createDirectory(at: directory, withIntermediateDirectories: true)
    var values = URLResourceValues()
    values.isExcludedFromBackup = true
    try directory.setResourceValues(values)
    try manager.setAttributes(
      [.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication],
      ofItemAtPath: directory.path
    )
    return directory
  }
}
