use std::fs;

use autosar_ecuc_bridge::ops;
use autosar_ecuc_bridge::project::SessionConfig;
use serde_json::json;
use tempfile::tempdir;

fn fixture() -> (tempfile::TempDir, SessionConfig) {
    let root = tempdir().expect("tempdir");
    let project = root.path().join("Cfg");
    let tool = root.path().join("SIP");
    fs::create_dir_all(project.join("Config/ECUC")).expect("project dirs");
    fs::create_dir_all(tool.join("BSWMD/Com")).expect("tool dirs");
    fs::write(
        project.join("Test.dpa"),
        r#"<?xml version="1.0"?>
<ProjectAssistant>
  <EcucSplitter>
    <Splitter File=".\Config\ECUC\Test_Com_ecuc.arxml">
      <Module Name="Com"/>
    </Splitter>
  </EcucSplitter>
</ProjectAssistant>"#,
    )
    .expect("dpa");
    fs::write(
        project.join("Config/ECUC/Test_Com_ecuc.arxml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<AUTOSAR>
  <ECUC-MODULE-CONFIGURATION-VALUES>
    <SHORT-NAME>Com</SHORT-NAME>
    <DEFINITION-REF DEST="ECUC-MODULE-DEF">/MICROSAR/Com</DEFINITION-REF>
    <CONTAINERS>
      <ECUC-CONTAINER-VALUE>
        <SHORT-NAME>ComConfig</SHORT-NAME>
        <DEFINITION-REF DEST="ECUC-PARAM-CONF-CONTAINER-DEF">/MICROSAR/Com/ComConfig</DEFINITION-REF>
        <SUB-CONTAINERS>
          <ECUC-CONTAINER-VALUE>
            <SHORT-NAME>SignalA</SHORT-NAME>
            <DEFINITION-REF DEST="ECUC-PARAM-CONF-CONTAINER-DEF">/MICROSAR/Com/ComConfig/ComSignal</DEFINITION-REF>
            <PARAMETER-VALUES>
              <ECUC-NUMERICAL-PARAM-VALUE>
                <DEFINITION-REF DEST="ECUC-INTEGER-PARAM-DEF">/MICROSAR/Com/ComConfig/ComSignal/ComBitPosition</DEFINITION-REF>
                <VALUE>8</VALUE>
              </ECUC-NUMERICAL-PARAM-VALUE>
            </PARAMETER-VALUES>
          </ECUC-CONTAINER-VALUE>
        </SUB-CONTAINERS>
      </ECUC-CONTAINER-VALUE>
    </CONTAINERS>
  </ECUC-MODULE-CONFIGURATION-VALUES>
</AUTOSAR>
"#,
    )
    .expect("config");
    fs::write(
        tool.join("BSWMD/Com/Com_bswmd.arxml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<AUTOSAR>
  <AR-PACKAGES>
    <AR-PACKAGE>
      <SHORT-NAME>MICROSAR</SHORT-NAME>
      <ELEMENTS>
        <ECUC-MODULE-DEF>
          <SHORT-NAME>Com</SHORT-NAME>
          <CONTAINERS>
            <ECUC-PARAM-CONF-CONTAINER-DEF>
              <SHORT-NAME>ComConfig</SHORT-NAME>
              <SUB-CONTAINERS>
                <ECUC-PARAM-CONF-CONTAINER-DEF>
                  <SHORT-NAME>ComSignal</SHORT-NAME>
                  <PARAMETERS>
                    <ECUC-INTEGER-PARAM-DEF>
                      <SHORT-NAME>ComBitPosition</SHORT-NAME>
                      <DESC><L-2 L="EN">Starting position in the I-PDU.</L-2></DESC>
                      <DEFAULT-VALUE>0</DEFAULT-VALUE>
                      <MIN>0</MIN>
                      <MAX>65535</MAX>
                    </ECUC-INTEGER-PARAM-DEF>
                  </PARAMETERS>
                </ECUC-PARAM-CONF-CONTAINER-DEF>
              </SUB-CONTAINERS>
            </ECUC-PARAM-CONF-CONTAINER-DEF>
          </CONTAINERS>
        </ECUC-MODULE-DEF>
      </ELEMENTS>
    </AR-PACKAGE>
  </AR-PACKAGES>
</AUTOSAR>"#,
    )
    .expect("template");
    fs::write(
        project.join("ecuc-bridge.json"),
        format!(
            "{{\"project_path\":{},\"tool_path\":{}}}",
            serde_json::to_string(&project).expect("project JSON"),
            serde_json::to_string(&tool).expect("tool JSON")
        ),
    )
    .expect("config file");
    let config = SessionConfig::load(&project).expect("load session");
    (root, config)
}

#[test]
fn accepts_legacy_configuration_for_migration() {
    let root = tempdir().expect("tempdir");
    let project = root.path().join("Cfg");
    let tool = root.path().join("SIP");
    fs::create_dir_all(&project).expect("project directory");
    fs::create_dir_all(&tool).expect("tool directory");
    fs::write(
        project.join("gyx-vector.json"),
        format!(
            "{{\"gyx_project_path\":{},\"gyx_tool_path\":{}}}",
            serde_json::to_string(&project).expect("project JSON"),
            serde_json::to_string(&tool).expect("tool JSON")
        ),
    )
    .expect("legacy config file");

    let config = SessionConfig::load(&project).expect("load legacy session");
    assert_eq!(config.project_path.file_name().unwrap(), "Cfg");
    assert!(!config.project_path.to_string_lossy().starts_with(r"\\?\"));
}

#[test]
fn finds_module_and_definition() {
    let (_root, config) = fixture();
    let module =
        ops::find_module::execute(&config, &json!({"module": "Com"})).expect("find module");
    assert_eq!(module["module"], "Com");

    let definition = ops::get_param_definition::execute(
        &config,
        &json!({"module": "Com", "params": "ComBitPosition"}),
    )
    .expect("get definition");
    assert_eq!(
        definition["definitions"][0]["definition_ref"],
        "/MICROSAR/Com/ComConfig/ComSignal/ComBitPosition"
    );
    assert_eq!(
        definition["definitions"][0]["value_tag"],
        "ECUC-NUMERICAL-PARAM-VALUE"
    );
}

#[test]
fn locates_container_by_definition_and_name() {
    let (_root, config) = fixture();
    let result = ops::locate_container::execute(
        &config,
        &json!({
            "module": "Com",
            "definition_ref": "/MICROSAR/Com/ComConfig/ComSignal",
            "short_name_regex": "^SignalA$"
        }),
    )
    .expect("locate");
    assert_eq!(result["count"], 1);
    assert_eq!(result["containers"][0]["short_name"], "SignalA");
    assert!(
        result["containers"][0]["start_line"].as_u64().unwrap()
            < result["containers"][0]["end_line"].as_u64().unwrap()
    );
}

#[test]
fn edits_only_requested_line_and_preserves_crlf() {
    let (_root, config) = fixture();
    let file = config.project_path.join("edit.arxml");
    fs::write(&file, b"one\r\ntwo\r\nthree\r\n").expect("write edit file");
    let result = ops::edit_file::execute(
        &config,
        &json!({
            "path": file,
            "edits": {"2": "TWO"}
        }),
    )
    .expect("edit");
    assert_eq!(result["applied_edits"], 1);
    assert_eq!(
        fs::read_to_string(config.project_path.join("edit.arxml")).expect("read edited"),
        "one\r\nTWO\r\nthree\r\n"
    );
}

#[test]
fn refuses_edit_outside_project() {
    let (root, config) = fixture();
    let outside = root.path().join("outside.arxml");
    fs::write(&outside, "one\n").expect("outside");
    let error = ops::edit_file::execute(
        &config,
        &json!({"path": outside, "edits": {"1": "changed"}}),
    )
    .expect_err("outside edit must fail");
    assert!(error.to_string().contains("outside project_path"));
}

#[cfg(windows)]
#[test]
fn session_paths_are_accepted_by_legacy_java_tools() {
    let (root, _config) = fixture();
    let project = root.path().join("Cfg");
    let tool = root.path().join("SIP");
    fs::write(
        project.join("gyx-vector.json"),
        format!(
            "{{\"gyx_project_path\":{},\"gyx_tool_path\":{}}}",
            serde_json::to_string(&project).expect("project JSON"),
            serde_json::to_string(&tool).expect("tool JSON")
        ),
    )
    .expect("config file");

    let config = SessionConfig::load(&project).expect("load session");
    assert!(!config.project_path.to_string_lossy().starts_with(r"\\?\"));
    assert!(!config
        .dpa_file()
        .expect("dpa")
        .to_string_lossy()
        .starts_with(r"\\?\"));
}
