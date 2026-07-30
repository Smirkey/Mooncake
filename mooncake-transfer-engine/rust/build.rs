// Copyright 2024 KVCache.AI
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=MOONCAKE_BUILD_DIR");
    println!("cargo:rerun-if-env-changed=MOONCAKE_WITH_CUDA");
    println!("cargo:rerun-if-env-changed=MOONCAKE_WITH_EFA");
    let configured_build = env::var_os("MOONCAKE_BUILD_DIR").map(PathBuf::from);
    let owned_build = if configured_build.is_none() && target_os() == "linux" {
        Some(build_native().unwrap_or_else(|error| panic!("{error}")))
    } else {
        None
    };
    if let Some(build) = configured_build.as_deref().or(owned_build.as_deref()) {
        // Top-level Mooncake build.
        link_search(build.join("mooncake-transfer-engine/src"));
        link_search(build.join("mooncake-transfer-engine/src/common/base"));
        // Standalone mooncake-transfer-engine build.
        link_search(build.join("src"));
        link_search(build.join("src/common/base"));
        link_search(build.join("mooncake-common-src"));
        // Shared ASIO output in both layouts.
        link_search(build.join("mooncake-common"));
    } else {
        link_search("../build/src");
        link_search("../../build/mooncake-transfer-engine/src");
        link_search("../build/src/common/base");
        link_search("../../build/mooncake-transfer-engine/src/common/base");
        link_search("../build/mooncake-common-src");
        link_search("../../build/mooncake-common-src");
        link_search("../build/mooncake-common");
        link_search("../../build/mooncake-common");
    }
    println!("cargo:rustc-link-lib=static=transfer_engine");

    // libbase.a holds mooncake::Status, which libtransfer_engine.a references.
    println!("cargo:rustc-link-lib=static=base");
    println!("cargo:rustc-link-lib=static=mooncake_common");

    println!("cargo:rustc-link-lib=static=asio_static");

    // EFA on AWS installs libfabric under /opt/amazon/efa/lib.
    if std::path::Path::new("/opt/amazon/efa/lib").exists() {
        println!("cargo:rustc-link-search=native=/opt/amazon/efa/lib");
    }

    println!("cargo:rustc-link-lib=stdc++");
    println!("cargo:rustc-link-lib=ibverbs");
    if flag_on("MOONCAKE_WITH_EFA") {
        println!("cargo:rustc-link-lib=fabric");
    }
    println!("cargo:rustc-link-lib=glog");
    println!("cargo:rustc-link-lib=gflags");
    println!("cargo:rustc-link-lib=pthread");
    println!("cargo:rustc-link-lib=jsoncpp");
    println!("cargo:rustc-link-lib=yaml-cpp");
    println!("cargo:rustc-link-lib=numa");
    println!("cargo:rustc-link-lib=curl");

    // etcd-cpp-api: only needed when transfer_engine was built with
    // USE_ETCD=ON.  Opt-in via MOONCAKE_WITH_ETCD=1 to keep non-etcd builds
    // (e.g. EFA-only on AWS) linkable.
    if flag_on("MOONCAKE_WITH_ETCD") {
        println!("cargo:rustc-link-lib=etcd-cpp-api");
    }

    // CUDA runtime: libtransfer_engine.a built with USE_CUDA=ON pulls in
    // cudaMemcpy/cudaStream* symbols.  The Rust demos themselves don't call
    // CUDA — this is purely a transitive archive dep.  Enable with
    // MOONCAKE_WITH_CUDA=1 and optional CUDA_HOME override for lib path.
    if owned_build.is_some() || flag_on("MOONCAKE_WITH_CUDA") {
        // Accept either a CUDA_HOME (append lib64/lib) or an explicit
        // CUDART_LIB_DIR that already points at the directory containing
        // libcudart.so.  This covers both /usr/local/cuda installs and
        // pip-wheel layouts like .../nvidia/cu13/lib.
        if let Ok(dir) = env::var("CUDART_LIB_DIR") {
            println!("cargo:rustc-link-search=native={}", dir);
        } else if let Ok(cuda_home) = env::var("CUDA_HOME") {
            let lib64 = PathBuf::from(&cuda_home).join("lib64");
            let lib = PathBuf::from(&cuda_home).join("lib");
            if lib64.exists() {
                println!("cargo:rustc-link-search=native={}", lib64.display());
            }
            if lib.exists() {
                println!("cargo:rustc-link-search=native={}", lib.display());
            }
        } else {
            println!("cargo:rustc-link-search=native=/usr/local/cuda/lib64");
        }
        println!("cargo:rustc-link-lib=cuda");
        println!("cargo:rustc-link-lib=cudart");
        println!("cargo:rustc-link-lib=mlx5");
        println!("cargo:rustc-link-lib=rt");
    }
}

fn link_search(path: impl AsRef<Path>) {
    println!("cargo:rustc-link-search=native={}", path.as_ref().display());
}

fn target_os() -> String {
    env::var("CARGO_CFG_TARGET_OS").unwrap_or_default()
}

fn flag_on(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            value == "1" || value.eq_ignore_ascii_case("on") || value.eq_ignore_ascii_case("true")
        })
        .unwrap_or(false)
}

fn build_native() -> Result<PathBuf, String> {
    let manifest = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .ok_or_else(|| "CARGO_MANIFEST_DIR is not set".to_owned())?,
    );
    let source = [
        manifest
            .parent()
            .map(Path::to_path_buf)
            .filter(|path| path.join("CMakeLists.txt").is_file()),
        Some(manifest.join("mooncake-transfer-engine"))
            .filter(|path| path.join("CMakeLists.txt").is_file()),
    ]
    .into_iter()
    .flatten()
    .next()
    .ok_or_else(|| {
        format!(
            "Mooncake native sources are unavailable beside {}",
            manifest.display()
        )
    })?;
    let build =
        PathBuf::from(env::var_os("OUT_DIR").ok_or_else(|| "OUT_DIR is not set".to_owned())?)
            .join("native");
    let mut configure = Command::new("cmake");
    configure
        .arg("-S")
        .arg(&source)
        .arg("-B")
        .arg(&build)
        .args([
            "-DCMAKE_BUILD_TYPE=Release",
            "-DBUILD_BENCHMARK=OFF",
            "-DBUILD_EXAMPLES=OFF",
            "-DBUILD_UNIT_TESTS=OFF",
            "-DENABLE_DEBUG_SYMBOLS=OFF",
            "-DUSE_CUDA=ON",
            "-DUSE_HTTP=OFF",
            "-DUSE_TCP=ON",
            "-DWITH_METRICS=OFF",
            "-DWITH_RUST_EXAMPLE=OFF",
            "-DWITH_STORE_C_SHARED=ON",
        ]);
    if flag_on("MOONCAKE_WITH_EFA") {
        configure.arg("-DUSE_EFA=ON");
    }
    run(&mut configure, "configure Mooncake Transfer Engine")?;

    let mut compile = Command::new("cmake");
    compile
        .arg("--build")
        .arg(&build)
        .args(["--target", "transfer_engine", "asio_static", "--parallel"])
        .arg(env::var("NUM_JOBS").unwrap_or_else(|_| "1".to_owned()));
    run(&mut compile, "build Mooncake Transfer Engine")?;
    Ok(build)
}

fn run(command: &mut Command, description: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("{description}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{description} exited with {status}"))
    }
}
