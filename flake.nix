{
  description = "MCP Support for Apollo Tooling";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/release-25.05";
    unstable.url = "github:nixos/nixpkgs/nixpkgs-unstable";

    # Helper utility for keeping certain paths from garbage collection in CI
    cache-nix-action = {
      url = "github:nix-community/cache-nix-action";
      flake = false;
    };

    # Rust builder
    crane.url = "github:ipetkov/crane";

    # Rust toolchains
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "unstable";
    };

    # Overlay for common architecture support
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    cache-nix-action,
    crane,
    flake-utils,
    nixpkgs,
    rust-overlay,
    unstable,
  } @ inputs:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = nixpkgs.legacyPackages.${system};
      unstable-pkgs = import unstable {
        inherit system;
        overlays = [(import rust-overlay)];

        # Elastic license is non-free, so we allow it to build here
        config.allowUnfreePredicate = pkg: let lib = unstable-pkgs.lib; in lib.strings.hasPrefix "apollo-" (lib.getName pkg);
      };

      # Define the toolchain based on the rust-toolchain file
      toolchain = unstable-pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      apollo-mcp-builder = unstable-pkgs.callPackage ./nix/apollo-mcp.nix {
        inherit crane toolchain;
      };

      # Supporting tools
      mcphost = pkgs.callPackage ./nix/mcphost.nix {};
      mcp-server-tools = pkgs.callPackage ./nix/mcp-server-tools {};

      # CI options
      garbageCollector = import "${inputs.cache-nix-action}/saveFromGC.nix" {
        inherit pkgs inputs;
        derivations = [mcphost] ++ apollo-mcp-builder.cache ++ mcp-server-tools;
      };
    in rec {
      devShells.default = pkgs.mkShell {
        nativeBuildInputs = apollo-mcp-builder.nativeDependencies;
        buildInputs =
          [
            mcphost
            toolchain
          ]
          ++ apollo-mcp-builder.dependencies
          ++ mcp-server-tools
          ++ (with pkgs; [
            # For running github action workflows locally
            act

            # For autogenerating nix evaluations for MCP server tools
            node2nix

            # Some of the mcp tooling likes to spawn arbitrary node runtimes,
            # so we need nodejs in the path here :(
            nodejs_22

            # For local LLM testing
            ollama

            # For consistent TOML formatting
            taplo

            # To allow using dependencies from git repositories in Cargo.toml
            git
          ]);
      };

      checks =
        {
          # Check formatting
          nix-fmt = pkgs.runCommandLocal "check-nix-fmt" {} "${pkgs.alejandra}/bin/alejandra --check ${./.}; touch $out";
        }
        // apollo-mcp-builder.checks;

      packages = let
        # Cross targets for supported architectures
        cross = let
          # Only Linux targets for Ubuntu CI builds
          supportedTargets = [
            "aarch64-unknown-linux-gnu"
            "x86_64-unknown-linux-gnu"
          ];

          crossBuild = target: let
            crossToolchain = toolchain.override {
              targets = [target];
            };
            apollo-mcp-cross = unstable-pkgs.callPackage ./nix/apollo-mcp.nix {
              inherit crane;
              toolchain = crossToolchain;
            };
          in
            apollo-mcp-cross.packages.builder target;
        in
          builtins.listToAttrs (builtins.map (target: {
              name = "cross-${target}";
              value = crossBuild target;
            })
            supportedTargets);
      in
        rec {
          inherit (garbageCollector) saveFromGC;

          default = apollo-mcp;
          apollo-mcp = apollo-mcp-builder.packages.apollo-mcp;
        }
        // cross;

      # TODO: This does not work on macOS without cross compiling, so maybe
      # we need to disable flake-utils and manually specify the supported
      # hosts?
      apps = let
        # Nix flakes don't yet expose a nice formatted timestamp in ISO-8601
        # format, so we need to drop out to date to do so.
        commitDate = pkgs.lib.readFile "${pkgs.runCommand "git-timestamp" {env.when = self.lastModified;} "echo -n `date -d @$when --iso-8601=seconds` > $out"}";

        # The architecture follows the docker format, which is not what nix uses
        archMapper = arch:
          builtins.getAttr arch {
            "aarch64" = "arm64";
            "x86_64" = "amd64";
          };
        builder = pkgs.dockerTools.streamLayeredImage {
          name = "apollo-mcp-server";
          tag = archMapper pkgs.stdenv.hostPlatform.uname.processor;

          # Use the latest commit time for reproducible builds
          created = commitDate;
          mtime = commitDate;

          contents = [
            packages.apollo-mcp
            pkgs.cacert
          ];

          # The server expects /data to exist, so we create it in the last layer to
          # ensure that the server doesn't crash if nothing is mounted.
          fakeRootCommands = ''
            mkdir data
            chmod a+r data
          '';

          # Image configuration
          # See: https://github.com/moby/moby/blob/46f7ab808b9504d735d600e259ca0723f76fb164/image/spec/spec.md#container-runconfig-field-descriptions
          config = let
            http-port = 5000;
          in {
            # Provide default options that can be unset / overridden by the end-user
            Env = [
              # Use Streamable HTTP transport by default, bound to all addresses
              "APOLLO_MCP_TRANSPORT__TYPE=streamable_http"
              "APOLLO_MCP_TRANSPORT__ADDRESS=0.0.0.0"
              "APOLLO_MCP_TRANSPORT__PORT=${builtins.toString http-port}"
            ];
            WorkingDir = "/data";

            # Make the entrypoint the server and have it read the config from /dev/null by default
            Cmd = ["/dev/null"];
            Entrypoint = [
              "apollo-mcp-server"
            ];

            # Listen on container port for Streamable HTTP requests
            ExposedPorts = {
              "${builtins.toString http-port}/tcp" = {};
            };

            # Drop to local user
            User = "1000";
            Group = "1000";
          };
        };
      in {
        streamImage = {
          type = "app";
          program = "${builder}";
          meta.description = "Builds the apollo-mcp-server container and streams the image to stdout.";
        };
      };
    });
}
