{ config, lib, pkgs, ... }:

let
  cfg = config.programs.kpick;
  tomlFormat = pkgs.formats.toml { };
in
{
  options.programs.kpick = {
    enable = lib.mkEnableOption "kpick, a KeePassXC password picker for Wayland";

    package = lib.mkPackageOption pkgs "kpick" { };

    settings = lib.mkOption {
      type = lib.types.submodule {
        freeformType = tomlFormat.type;

        options = {
          database_path = lib.mkOption {
            type = lib.types.nullOr lib.types.str;
            default = null;
            example = "~/Documents/passwords.kdbx";
            description = "Path to the KeePass database file.";
          };

          clipboard_timeout = lib.mkOption {
            type = lib.types.ints.unsigned;
            default = 10;
            description = "Seconds before clipboard is cleared (0 = never).";
          };

          flash_duration = lib.mkOption {
            type = lib.types.ints.unsigned;
            default = 150;
            description = "Milliseconds to show the input flash indicator.";
          };

          window = lib.mkOption {
            type = lib.types.submodule {
              options = {
                password = lib.mkOption {
                  type = lib.types.submodule {
                    options = {
                      width = lib.mkOption {
                        type = lib.types.ints.positive;
                        default = 400;
                        description = "Password prompt width in pixels.";
                      };
                      height = lib.mkOption {
                        type = lib.types.ints.positive;
                        default = 172;
                        description = "Password prompt height in pixels.";
                      };
                      max_percent = lib.mkOption {
                        type = lib.types.ints.between 1 100;
                        default = 40;
                        description = "Maximum percentage of screen for password window.";
                      };
                    };
                  };
                  default = { };
                  description = "Password prompt window settings.";
                };

                picker = lib.mkOption {
                  type = lib.types.submodule {
                    options = {
                      width_percent = lib.mkOption {
                        type = lib.types.ints.between 1 100;
                        default = 50;
                        description = "Picker width as percentage of screen.";
                      };
                      height_percent = lib.mkOption {
                        type = lib.types.ints.between 1 100;
                        default = 40;
                        description = "Picker height as percentage of screen.";
                      };
                      max_entries = lib.mkOption {
                        type = lib.types.ints.positive;
                        default = 10;
                        description = "Maximum visible entries in picker.";
                      };
                    };
                  };
                  default = { };
                  description = "Picker window settings.";
                };
              };
            };
            default = { };
            description = "Window configuration.";
          };

          font = lib.mkOption {
            type = lib.types.submodule {
              options = {
                family = lib.mkOption {
                  type = lib.types.str;
                  default = "DejaVu Sans";
                  description = "Font family name.";
                };
                size = lib.mkOption {
                  type = lib.types.number;
                  default = 18.0;
                  description = "Main font size in pixels.";
                };
                hints_size = lib.mkOption {
                  type = lib.types.number;
                  default = 14.0;
                  description = "Hints font size in pixels.";
                };
              };
            };
            default = { };
            description = "Font settings.";
          };

          colors = lib.mkOption {
            type = lib.types.submodule {
              options = {
                background = lib.mkOption {
                  type = lib.types.str;
                  default = "#1e1e1e";
                  description = "Background color.";
                };
                background_light = lib.mkOption {
                  type = lib.types.str;
                  default = "#2d2d2d";
                  description = "Light background color.";
                };
                selection = lib.mkOption {
                  type = lib.types.str;
                  default = "#264f78";
                  description = "Selection highlight color.";
                };
                foreground = lib.mkOption {
                  type = lib.types.str;
                  default = "#cccccc";
                  description = "Main text color.";
                };
                foreground_subtle = lib.mkOption {
                  type = lib.types.str;
                  default = "#6e6e6e";
                  description = "Subtle/dimmed text color.";
                };
                foreground_bright = lib.mkOption {
                  type = lib.types.str;
                  default = "#ffffff";
                  description = "Bright/highlighted text color.";
                };
                error = lib.mkOption {
                  type = lib.types.str;
                  default = "#ff6b6b";
                  description = "Error message color.";
                };
              };
            };
            default = { };
            description = "Color scheme.";
          };
        };
      };
      default = { };
      description = ''
        Configuration for kpick. See the kpick documentation for available options.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];

    xdg.dataFile."kpick/config.toml" = lib.mkIf (cfg.settings != { }) {
      source = tomlFormat.generate "kpick-config" cfg.settings;
    };
  };
}
