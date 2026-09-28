import 'package:geocoding/geocoding.dart';
import 'package:geolocator/geolocator.dart';

/// Resolve a foreground, approximate position on the phone. Only the city
/// label leaves the device; coordinates are neither uploaded nor retained.
Future<String?> askDeviceCity() async {
  try {
    if (!await Geolocator.isLocationServiceEnabled()) return null;
    var permission = await Geolocator.checkPermission();
    if (permission == LocationPermission.denied) {
      permission = await Geolocator.requestPermission();
    }
    if (permission != LocationPermission.whileInUse &&
        permission != LocationPermission.always) {
      return null;
    }
    final position = await Geolocator.getCurrentPosition(
      locationSettings: const LocationSettings(
        accuracy: LocationAccuracy.low,
        timeLimit: Duration(seconds: 5),
      ),
    );
    final places = await Geocoding()
        .placemarkFromCoordinates(position.latitude, position.longitude)
        .timeout(const Duration(seconds: 4));
    for (final place in places) {
      final city = place.locality?.trim() ?? '';
      if (city.isNotEmpty) return city;
    }
  } catch (_) {
    // Ask remains available with an explicit city or all locations.
  }
  return null;
}
