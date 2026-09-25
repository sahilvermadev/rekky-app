package app.rekky.rekky_flutter

import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import java.io.File

class MainActivity : FlutterActivity() {
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "app.rekky/voice_storage")
            .setMethodCallHandler { call, result ->
                val directory = File(noBackupFilesDir, "rekky_voice_drafts_v2")
                when (call.method) {
                    "directory" -> {
                        if (directory.isDirectory || directory.mkdirs()) {
                            result.success(directory.absolutePath)
                        } else {
                            result.error("storage", "Cannot create protected voice storage", null)
                        }
                    }
                    "protectFile" -> {
                        val path = call.argument<String>("path")
                        val file = path?.let { File(it).canonicalFile }
                        if (file != null && file.parentFile == directory.canonicalFile && file.isFile) {
                            result.success(null) // noBackupFilesDir is excluded by Android.
                        } else {
                            result.error("storage", "Voice file is outside protected storage", null)
                        }
                    }
                    else -> result.notImplemented()
                }
            }
    }
}
