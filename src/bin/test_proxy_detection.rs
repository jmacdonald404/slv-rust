use slv_rust::ui::proxy::ProxySettings;

fn main() {
    println!("🔍 Testing Hippolyzer proxy detection...");
    
    // Test proxy detection
    let proxy_detected = ProxySettings::detect_hippolyzer_proxy();
    
    println!("🔍 Proxy detection result: {}", proxy_detected);
    
    if proxy_detected {
        println!("✅ SUCCESS: Hippolyzer proxy is available");
        println!("   HTTP proxy: 127.0.0.1:9062");
        println!("   SOCKS5 proxy: 127.0.0.1:9061");
    } else {
        println!("❌ FAIL: Hippolyzer proxy not detected");
        println!("   Make sure Hippolyzer is running on ports 9061 and 9062");
    }
    
    // Test default proxy settings
    let default_settings = ProxySettings::default();
    println!("\n📋 Default proxy settings:");
    println!("   Enabled: {}", default_settings.enabled);
    println!("   HTTP: {}:{}", default_settings.http_host, default_settings.http_port);
    println!("   SOCKS5: {}:{}", default_settings.socks5_host, default_settings.socks5_port);
}