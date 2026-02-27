{ pkgs ? import <nixpkgs> {} }:

pkgs.rustPlatform.buildRustPackage rec {
  pname = "kaede-beta";
  version = "1.0.1";

  src = ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  nativeBuildInputs = with pkgs; [
    pkg-config
    wrapGAppsHook4
  ];

  buildInputs = with pkgs; [
    gtk4
    libadwaita
  ];

  postInstall = ''
    install -Dm644 com.kaede.gpu-manager.desktop $out/share/applications/com.kaede.gpu-manager.desktop
    install -Dm644 src/icons/icon.png $out/share/icons/hicolor/256x256/apps/com.kaede.gpu-manager.png
  '';

  meta = with pkgs.lib; {
    description = "Linux desktop app to choose which GPU to use per app/game";
    homepage = "https://github.com/SterTheStar/kaede";
    license = licenses.gpl3Only;
    platforms = platforms.linux;
  };
}
