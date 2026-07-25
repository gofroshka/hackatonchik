# Sonic Share Mobile

Flutter client for transferring files and chat messages through the acoustic
protocol implemented by `../core`.

The native adapter lives in `rust/` and is built through the generated
Flutter Rust Bridge/Cargokit plugin in `rust_builder/`.

```bash
flutter pub get
flutter test
flutter build apk --debug
flutter build ios --debug --no-codesign
```
