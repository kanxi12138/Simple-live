"""Run the production APK identity validator with the cached Kotlin compiler."""
import os
from pathlib import Path
import subprocess


def main() -> None:
    root = Path(__file__).resolve().parent.parent
    source = (root / 'app/app/src/main/java/www/sp/com/MainActivity.kt').read_text(encoding='utf-8')
    validator = source.split('internal fun validateUpdateIdentity(', 1)[1].split('class MainActivity', 1)[0]
    output = root / 'web/src-tauri/target/repair-kotlin'
    output.mkdir(parents=True, exist_ok=True)
    fixture = output / 'InstallPolicyCheck.kt'
    fixture.write_text('internal fun validateUpdateIdentity(' + validator + '''
fun main() {
  fun validate(name: String = "www.sp.com", version: Long = 5002002,
    signers: Set<String>? = setOf("fixture")): String =
    validateUpdateIdentity(name, "www.sp.com", version, 5002002, "5.2.2", signers, setOf("fixture"))
  check(validate().isEmpty())
  check(validate(name = "other.package").contains("包名"))
  check(validate(version = 5002001).contains("版本"))
  check(validate(signers = setOf("different")).contains("签名"))
  check(validate(signers = emptySet()).contains("签名"))
  println("PASS: APK package, version and signing identity")
}
''', encoding='utf-8')
    cache = Path.home() / '.gradle/caches/modules-2/files-2.1'
    jars: list[Path] = []
    for artifact, version in [('kotlin-compiler-embeddable', '2.0.21'), ('kotlin-stdlib', '2.0.21'),
                              ('kotlin-script-runtime', '2.0.21'), ('kotlin-reflect', '1.6.10')]:
        jars.extend((cache / 'org.jetbrains.kotlin' / artifact / version).glob('*/*.jar'))
    jars.extend((cache / 'org.jetbrains.intellij.deps/trove4j').glob('*/*/*.jar'))
    jars.extend((cache / 'org.jetbrains/annotations').glob('*/*/*.jar'))
    jars.extend((cache / 'org.jetbrains.kotlinx/kotlinx-coroutines-core-jvm').glob('*/*/*.jar'))
    classpath = os.pathsep.join(map(str, jars))
    java = str(Path(os.environ['JAVA_HOME']) / 'bin/java.exe')
    subprocess.run([java, '-cp', classpath, 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',
                    '-no-stdlib', '-no-reflect', '-classpath', classpath,
                    '-d', str(output / 'classes'), str(fixture)], check=True)
    subprocess.run([java, '-cp', str(output / 'classes') + os.pathsep + classpath,
                    'InstallPolicyCheckKt'], check=True)


if __name__ == '__main__':
    main()
