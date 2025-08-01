#[cfg(test)]
mod comprehensive_tests {
    use super::*;
    use crate::utils::math::{Vector3, RegionHandle, parsing as math_parsing};

    #[test]
    fn test_vector3_parsing() {
        // Test various Second Life vector formats
        let vectors = vec![
            ("[r1.0, r0.0, r0.0]", Vector3::new(1.0, 0.0, 0.0)),
            ("r1,0,0", Vector3::new(1.0, 0.0, 0.0)),
            ("[r0.5, r-2.3, r10.7]", Vector3::new(0.5, -2.3, 10.7)),
        ];

        for (input, expected) in vectors {
            let result = Vector3::parse_sl_format(input);
            assert!(result.is_ok(), "Failed to parse vector: {}", input);
            assert_eq!(result.unwrap(), expected);
        }
    }

    #[test]
    fn test_region_handle_parsing() {
        // Test region handle parsing
        let handles = vec![
            ("[r123456, r789012]", RegionHandle::new(123456, 789012)),
            ("[r0, r0]", RegionHandle::new(0, 0)),
            ("[r-1000, r2000]", RegionHandle::new(-1000, 2000)),
        ];

        for (input, expected) in handles {
            let result = RegionHandle::parse_sl_format(input);
            assert!(result.is_ok(), "Failed to parse region handle: {}", input);
            assert_eq!(result.unwrap(), expected);
        }
    }

    #[test]
    fn test_math_parsing_utilities() {
        // Test boolean parsing
        assert_eq!(math_parsing::parse_bool("true").unwrap(), true);
        assert_eq!(math_parsing::parse_bool("false").unwrap(), false);
        assert_eq!(math_parsing::parse_bool("1").unwrap(), true);
        assert_eq!(math_parsing::parse_bool("0").unwrap(), false);

        // Test UUID parsing
        let test_uuid = "550e8400-e29b-41d4-a716-446655440000";
        let parsed_uuid = math_parsing::parse_uuid(test_uuid).unwrap();
        assert_eq!(parsed_uuid.to_string(), test_uuid);

        // Test number parsing
        assert_eq!(math_parsing::parse_number("123").unwrap(), 123.0);
        assert_eq!(math_parsing::parse_number("3.14").unwrap(), 3.14);

        // Test string array parsing
        let array = math_parsing::parse_string_array("a,b,c");
        assert_eq!(array, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    }

    #[test]
    fn test_login_response_creation() {
        let mut response = LoginResponse::default();
        
        // Test basic field setting
        response.success = true;
        response.first_name = "Test".to_string();
        response.last_name = "User".to_string();
        response.look_at = Vector3::new(1.0, 0.0, 0.0);
        
        assert!(response.is_successful());
        assert_eq!(response.full_name(), "Test User");
        assert_eq!(response.look_at, Vector3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn test_xmlrpc_field_parsing() {
        let client = XmlRpcClient::new();
        let mut response = LoginResponse::default();

        // Test various field types
        assert!(client.set_response_field(&mut response, "login", "true").is_ok());
        assert!(response.success);

        assert!(client.set_response_field(&mut response, "agent_id", "550e8400-e29b-41d4-a716-446655440000").is_ok());
        assert_eq!(response.agent_id.to_string(), "550e8400-e29b-41d4-a716-446655440000");

        assert!(client.set_response_field(&mut response, "look_at", "[r1.0, r0.0, r0.0]").is_ok());
        assert_eq!(response.look_at, Vector3::new(1.0, 0.0, 0.0));

        assert!(client.set_response_field(&mut response, "first_name", "\"Test\"").is_ok());
        assert_eq!(response.first_name, "Test");

        assert!(client.set_response_field(&mut response, "udp_blacklist", "server1,server2,server3").is_ok());
        assert_eq!(response.udp_blacklist, Some(vec!["server1".to_string(), "server2".to_string(), "server3".to_string()]));
    }

    #[test]
    fn test_comprehensive_response_parsing() {
        // Test a comprehensive login response with all fields
        let xml_response = r#"
        <?xml version="1.0"?>
        <methodResponse>
            <params>
                <param>
                    <value>
                        <struct>
                            <member>
                                <name>login</name>
                                <value><boolean>1</boolean></value>
                            </member>
                            <member>
                                <name>agent_id</name>
                                <value><string>550e8400-e29b-41d4-a716-446655440000</string></value>
                            </member>
                            <member>
                                <name>session_id</name>
                                <value><string>660e8400-e29b-41d4-a716-446655440000</string></value>
                            </member>
                            <member>
                                <name>secure_session_id</name>
                                <value><string>770e8400-e29b-41d4-a716-446655440000</string></value>
                            </member>
                            <member>
                                <name>first_name</name>
                                <value><string>"Test"</string></value>
                            </member>
                            <member>
                                <name>last_name</name>
                                <value><string>User</string></value>
                            </member>
                            <member>
                                <name>circuit_code</name>
                                <value><int>12345</int></value>
                            </member>
                            <member>
                                <name>sim_ip</name>
                                <value><string>127.0.0.1</string></value>
                            </member>
                            <member>
                                <name>sim_port</name>
                                <value><int>9000</int></value>
                            </member>
                            <member>
                                <name>look_at</name>
                                <value><string>[r1.0, r0.0, r0.0]</string></value>
                            </member>
                            <member>
                                <name>agent_access</name>
                                <value><string>M</string></value>
                            </member>
                            <member>
                                <name>max_agent_groups</name>
                                <value><int>25</int></value>
                            </member>
                            <member>
                                <name>udp_blacklist</name>
                                <value><string>server1,server2</string></value>
                            </member>
                        </struct>
                    </value>
                </param>
            </params>
        </methodResponse>
        "#;

        let client = XmlRpcClient::new();
        let result = client.parse_login_response(xml_response);
        
        assert!(result.is_ok(), "Failed to parse login response: {:?}", result.err());
        
        let response = result.unwrap();
        assert!(response.success);
        assert_eq!(response.first_name, "Test");
        assert_eq!(response.last_name, "User");
        assert_eq!(response.circuit_code, 12345);
        assert_eq!(response.simulator_ip, "127.0.0.1");
        assert_eq!(response.simulator_port, 9000);
        assert_eq!(response.look_at, Vector3::new(1.0, 0.0, 0.0));
        assert_eq!(response.agent_access, Some("M".to_string()));
        assert_eq!(response.max_agent_groups, Some(25));
        assert_eq!(response.udp_blacklist, Some(vec!["server1".to_string(), "server2".to_string()]));
    }

    #[test]
    fn test_error_response_parsing() {
        // Test error response parsing
        let xml_error = r#"
        <?xml version="1.0"?>
        <methodResponse>
            <params>
                <param>
                    <value>
                        <struct>
                            <member>
                                <name>login</name>
                                <value><boolean>0</boolean></value>
                            </member>
                            <member>
                                <name>reason</name>
                                <value><string>Invalid credentials</string></value>
                            </member>
                            <member>
                                <name>message</name>
                                <value><string>Login failed</string></value>
                            </member>
                        </struct>
                    </value>
                </param>
            </params>
        </methodResponse>
        "#;

        let client = XmlRpcClient::new();
        let result = client.parse_login_response(xml_error);
        
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(!response.success);
        assert_eq!(response.reason, Some("Invalid credentials".to_string()));
        assert_eq!(response.message, Some("Login failed".to_string()));
        assert_eq!(response.error_message(), Some("Invalid credentials"));
    }

    #[test]
    fn test_complex_field_parsing() {
        let client = XmlRpcClient::new();
        
        // Test home_info parsing
        let home_info_xml = r#"
        <struct>
            <member>
                <name>region_handle</name>
                <value><string>[r123456, r789012]</string></value>
            </member>
            <member>
                <name>position</name>
                <value><string>[r128.0, r128.0, r22.0]</string></value>
            </member>
            <member>
                <name>look_at</name>
                <value><string>[r1.0, r0.0, r0.0]</string></value>
            </member>
        </struct>
        "#;

        let doc = roxmltree::Document::parse(home_info_xml).unwrap();
        let struct_node = doc.root_element();
        let result = client.parse_complex_field(struct_node, "home_info");
        
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert!(parsed.contains("region_handle"));
        assert!(parsed.contains("position"));
        assert!(parsed.contains("look_at"));
    }
} 