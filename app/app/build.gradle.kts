import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.kotlin.kapt")
}
val localConfig = Properties().apply {
    val source = rootProject.file("local.properties")
    if (source.exists()) source.inputStream().use { load(it) }
}
android {
    namespace = "com.simplelive.nativeapp"
    compileSdk = 36
    defaultConfig {
        applicationId = "com.simplelive.nativeapp"
        minSdk = 24
        targetSdk = 36
        versionCode = 7000008
        versionName = "7.0.8"
    }
    signingConfigs {
        create("release") {
            localConfig.getProperty("release.keystore.path")?.let { storeFile = file(it) }
            storePassword = localConfig.getProperty("release.keystore.storePassword")
            keyAlias = localConfig.getProperty("release.keystore.keyAlias")
            keyPassword = localConfig.getProperty("release.keystore.keyPassword")
        }
    }
    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("release")
        }
    }
    sourceSets.getByName("main").java.setSrcDirs(listOf("src/main/kotlin"))
    sourceSets.getByName("main").jniLibs.setSrcDirs(emptyList<String>())
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    buildFeatures { compose = true; buildConfig = true }
    packaging { resources.excludes += setOf("META-INF/DEPENDENCIES", "META-INF/LICENSE*", "META-INF/NOTICE*") }
}
dependencies {
    implementation(platform("androidx.compose:compose-bom:2026.02.01"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.9.4")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.9.4")
    implementation("androidx.core:core-ktx:1.16.0")
    implementation("androidx.room:room-runtime:2.7.2")
    implementation("androidx.room:room-ktx:2.7.2")
    kapt("androidx.room:room-compiler:2.7.2")
    implementation("androidx.datastore:datastore-preferences:1.1.7")
    implementation("androidx.media3:media3-exoplayer:1.11.0")
    implementation("androidx.media3:media3-exoplayer-hls:1.11.0")
    implementation("androidx.media3:media3-ui:1.11.0")
    implementation("androidx.media3:media3-datasource-okhttp:1.11.0")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("com.neovisionaries:nv-websocket-client:2.14")
    implementation("io.coil-kt:coil-compose:2.7.0")
    implementation("org.mozilla:rhino:1.7.15")
    implementation("org.brotli:dec:0.1.2")
    implementation("org.jsoup:jsoup:1.18.3")
}
