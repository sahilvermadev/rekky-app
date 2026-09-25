import 'dart:io';

import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:google_sign_in/google_sign_in.dart';
import 'package:sign_in_with_apple/sign_in_with_apple.dart';

class IdentityService {
  static const storage = FlutterSecureStorage();
  static const _sessionKey = 'rekky_session_v1';
  static const googleClientId = String.fromEnvironment('GOOGLE_CLIENT_ID');
  static const googleServerClientId = String.fromEnvironment(
    'GOOGLE_SERVER_CLIENT_ID',
  );

  Future<String?> storedToken() => storage.read(key: _sessionKey);
  Future<void> storeToken(String token) =>
      storage.write(key: _sessionKey, value: token);
  Future<void> clearToken() => storage.delete(key: _sessionKey);

  Future<String> googleIdToken() async {
    if (googleServerClientId.isEmpty) {
      throw StateError('Set GOOGLE_SERVER_CLIENT_ID for Google sign-in.');
    }
    if (Platform.isIOS && googleClientId.isEmpty) {
      throw StateError('Set GOOGLE_CLIENT_ID for iOS Google sign-in.');
    }
    final signIn = GoogleSignIn.instance;
    await signIn.initialize(
      clientId: googleClientId.isEmpty ? null : googleClientId,
      serverClientId: googleServerClientId,
    );
    final account = await signIn.authenticate();
    final token = account.authentication.idToken;
    if (token == null) {
      throw StateError('Google did not return an identity token.');
    }
    return token;
  }

  bool get supportsApple => Platform.isIOS;
  Future<String> appleIdToken() async {
    if (!Platform.isIOS || !await SignInWithApple.isAvailable()) {
      throw StateError('Apple sign-in is unavailable on this device.');
    }
    final credential = await SignInWithApple.getAppleIDCredential(
      scopes: [AppleIDAuthorizationScopes.email],
    );
    final token = credential.identityToken;
    if (token == null) {
      throw StateError('Apple did not return an identity token.');
    }
    return token;
  }
}
