ThisBuild / version := "1.0.0"
ThisBuild / scalaVersion := "3.3.1"
ThisBuild / organization := "io.github.weaveitmeta"

// Sonatype Central Portal publishing settings
ThisBuild / sonatypeCredentialHost := "central.sonatype.com"
ThisBuild / publishTo := sonatypePublishToBundle.value
ThisBuild / publishMavenStyle := true

// GPG settings
Global / PgpKeys.gpgCommand := "C:\\Program Files\\GnuPG\\bin\\gpg.exe"

lazy val root = (project in file("."))
  .settings(
    name := "qft-scala",
    description := "Production-grade Scala SDK for Quantum File Type (.qft) format - quantum state storage using MPS tensor networks",
    
    libraryDependencies ++= Seq(
      "com.google.code.gson" % "gson" % "2.10.1",
      "org.scalatest" %% "scalatest" % "3.2.17" % Test
    ),
    
    // Publish settings
    licenses := Seq(
      "MIT" -> url("https://opensource.org/licenses/MIT"),
      "Apache-2.0" -> url("https://www.apache.org/licenses/LICENSE-2.0")
    ),
    homepage := Some(url("https://github.com/WeaveITMeta/Quantum-Operating-System")),
    scmInfo := Some(
      ScmInfo(
        url("https://github.com/WeaveITMeta/Quantum-Operating-System"),
        "scm:git@github.com:WeaveITMeta/Quantum-Operating-System.git"
      )
    ),
    developers := List(
      Developer(
        id = "macrohard",
        name = "MACROHARD Quantum OS Team",
        email = "quantum@macrohard.dev",
        url = url("https://github.com/WeaveITMeta")
      )
    ),
    
    // PGP signing for Maven Central
    usePgpKeyHex("AA2F2F2F6555B646"),
    PgpKeys.gpgCommand := "C:\\Program Files\\GnuPG\\bin\\gpg.exe",
    
    // Sonatype bundle release
    sonatypeBundleDirectory := (ThisBuild / baseDirectory).value / "target" / "sonatype-staging"
  )
