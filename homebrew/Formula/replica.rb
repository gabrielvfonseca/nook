class Replica < Formula
  desc "Isolated APFS clonefile workspaces for AI agents"
  homepage "https://github.com/gabrielvfonseca/replica"
  version "0.1.0"
  license "Apache-2.0"
  head "https://github.com/gabrielvfonseca/replica.git", branch: "main"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/gabrielvfonseca/replica/releases/download/v#{version}/replica-darwin-arm64"
      sha256 "32ef76040e9035b9fdac6e87e7409b9c14a02952d3aecc4460084679ea84bb74"
    else
      url "https://github.com/gabrielvfonseca/replica/releases/download/v#{version}/replica-darwin-x86_64"
      sha256 "d4dc3fc21428af346b7602dc1ccc9634ca164908ec610eb1a81d2c08070359a6"
    end
  end

  depends_on "rust" => :build if build.head?

  def install
    if build.head?
      system "cargo", "install", *std_cargo_args
    else
      bin.install Hardware::CPU.arm? ? "replica-darwin-arm64" : "replica-darwin-x86_64" => "replica"
    end
  end

  service do
    run [opt_bin/"replica", "gc"]
    run_type :interval
    interval 600
    environment_variables REPLICA_ROOT: "#{Dir.home}/replica"
    log_path var/"log/replica-gc.log"
    error_log_path var/"log/replica-gc.log"
  end

  def caveats
    <<~EOS
      Replica needs REPLICA_ROOT on an APFS volume (clonefile cannot cross volumes).
      Add to your shell profile:
        export REPLICA_ROOT=/Volumes/Work/replica

      Start here:
        replica help quickstart

      To run `replica gc` every 10 minutes in the background:
        brew services start replica
      The service uses REPLICA_ROOT=$HOME/replica, because launchd does not read
      your shell profile. If your workspaces live elsewhere, run `replica gc`
      yourself from cron or launchd with your own REPLICA_ROOT.
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/replica --version")
    assert_match "USAGE", shell_output("#{bin}/replica help")
  end
end
