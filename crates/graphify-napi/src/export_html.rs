use std::collections::HashMap;
use std::path::Path;

use rusqlite::Connection;

pub fn export_html(db: &Connection, out_path: &Path) -> graphify_core::Result<()> {
    // Read nodes
    let mut nodes_json = Vec::new();
    let mut communities: HashMap<i64, usize> = HashMap::new();
    let mut next_color = 0;
    {
        let mut stmt = db.prepare(
            "SELECT id, label, file_type, source_file, source_line, community FROM nodes",
        )?;
        #[allow(clippy::type_complexity)]
        let rows: Vec<(String, String, String, String, Option<i64>, Option<i64>)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        for (id, label, ft, sf, line, comm) in &rows {
            let group = match comm {
                Some(c) => {
                    let c = *c;
                    if let std::collections::hash_map::Entry::Vacant(e) = communities.entry(c) {
                        e.insert(next_color);
                        next_color += 1;
                    }
                    *communities.get(&c).unwrap()
                }
                None => 0,
            };
            let node_obj = serde_json::json!({
                "id": id,
                "label": label,
                "group": group,
                "fileType": ft,
                "sourceFile": sf,
                "sourceLine": line,
            });
            nodes_json.push(serde_json::to_string(&node_obj).unwrap_or_default());
        }
    }

    // Read edges
    let mut edges_json = Vec::new();
    {
        let mut stmt = db.prepare(
            "SELECT source, target, relation, confidence FROM edges",
        )?;
        let rows: Vec<(String, String, String, String)> = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        for (src, tgt, rel, conf) in &rows {
            let edge_obj = serde_json::json!({
                "from": src,
                "to": tgt,
                "relation": rel,
                "confidence": conf,
            });
            edges_json.push(serde_json::to_string(&edge_obj).unwrap_or_default());
        }
    }

    // Generate color palette
    let num_colors = std::cmp::max(next_color, 1);
    let mut colors = Vec::new();
    for i in 0..num_colors {
        let hue = (i as f64 / num_colors as f64) * 360.0;
        colors.push(format!("hsl({}, 70%, 60%)", hue as i32));
    }
    let colors_js = if colors.is_empty() {
        "['#6baed6']".to_string()
    } else {
        format!(
            "[{}]",
            colors
                .iter()
                .map(|c| format!("'{}'", c))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    // Escape </script to prevent XSS when embedding JSON in HTML <script> tags
    let nodes_js = nodes_json.join(",\n        ").replace("</script", "<\\/script");
    let edges_js = edges_json.join(",\n        ").replace("</script", "<\\/script");

    let html = format!(
        r##"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>graphify &mdash; Knowledge Graph</title>
  <style>
    body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; margin: 0; padding: 0; background: #1a1a2e; color: #e0e0e0; }}
    #search {{ position: fixed; top: 12px; left: 12px; z-index: 100; }}
    #search input {{ width: 280px; padding: 8px 12px; border: 1px solid #444; border-radius: 6px; background: #16213e; color: #e0e0e0; font-size: 14px; }}
    #search input::placeholder {{ color: #888; }}
    #info {{ position: fixed; bottom: 12px; left: 12px; z-index: 100; background: #16213e; border: 1px solid #333; border-radius: 6px; padding: 10px 14px; max-width: 400px; font-size: 13px; display: none; }}
    #info .label {{ font-weight: bold; font-size: 15px; margin-bottom: 4px; }}
    #info .meta {{ color: #aaa; }}
    #legend {{ position: fixed; top: 12px; right: 12px; z-index: 100; background: #16213e; border: 1px solid #333; border-radius: 6px; padding: 10px 14px; font-size: 12px; max-height: 300px; overflow-y: auto; }}
    #legend h4 {{ margin: 0 0 6px 0; color: #ccc; }}
    #legend .item {{ display: flex; align-items: center; gap: 6px; margin: 2px 0; }}
    #legend .swatch {{ width: 14px; height: 14px; border-radius: 3px; }}
    #network {{ width: 100vw; height: 100vh; }}
  </style>
</head>
<body>
  <div id="search"><input type="text" id="searchInput" placeholder="Search nodes..." /></div>
  <div id="legend"></div>
  <div id="info"><div class="label" id="infoLabel"></div><div class="meta" id="infoMeta"></div></div>
  <div id="network"></div>
  <script src="https://unpkg.com/vis-network/standalone/umd/vis-network.min.js" integrity="sha384-OrA3tQDSHkPYtkfOSPGKkXik9gAWHb39oFSi0NN3rWsrDP8mxIQJMYi13B/+lDpf" crossorigin="anonymous"></script>
  <script>
    var colors = {colors_js};
    var nodes = new vis.DataSet([
        {nodes_js}
    ]);
    var edges = new vis.DataSet([
        {edges_js}
    ]);

    // Assign colors by group
    nodes.forEach(function(n) {{
      n.color = colors[n.group % colors.length];
      nodes.update(n);
    }});

    // Build legend
    var groups = {{}};
    nodes.forEach(function(n) {{ groups[n.group] = (groups[n.group] || 0) + 1; }});
    var legendHtml = '<h4>Communities</h4>';
    Object.keys(groups).sort(function(a,b) {{ return +a - +b; }}).forEach(function(g) {{
      legendHtml += '<div class="item"><div class="swatch" style="background:' + colors[g % colors.length] + '"></div> Community ' + g + ' (' + groups[g] + ' nodes)</div>';
    }});
    document.getElementById('legend').innerHTML = legendHtml;

    var container = document.getElementById('network');
    var data = {{ nodes: nodes, edges: edges }};
    var options = {{
      nodes: {{ shape: 'dot', size: 12, font: {{ color: '#e0e0e0', size: 11 }}, borderWidth: 1, borderWidthSelected: 2 }},
      edges: {{ color: {{ color: '#555', highlight: '#fff', hover: '#aaa' }}, smooth: {{ type: 'continuous' }}, arrows: {{ to: {{ enabled: true, scaleFactor: 0.5 }} }}, font: {{ color: '#888', size: 9, strokeWidth: 0 }} }},
      physics: {{ barnesHut: {{ gravitationalConstant: -3000, centralGravity: 0.3, springLength: 120 }}, stabilization: {{ iterations: 150 }} }},
      interaction: {{ hover: true, tooltipDelay: 200, navigationButtons: true, keyboard: true }}
    }};

    var network = new vis.Network(container, data, options);

    // Node info panel
    network.on('click', function(params) {{
      var info = document.getElementById('info');
      if (params.nodes.length > 0) {{
        var nodeId = params.nodes[0];
        var node = nodes.get(nodeId);
        document.getElementById('infoLabel').textContent = node.label;
        document.getElementById('infoMeta').textContent = node.sourceFile + (node.sourceLine ? ':' + node.sourceLine : '') + ' | ' + node.fileType + ' | community ' + node.group;
        info.style.display = 'block';
      }} else {{
        info.style.display = 'none';
      }}
    }});

    // Search
    var searchInput = document.getElementById('searchInput');
    searchInput.addEventListener('input', function() {{
      var q = this.value.toLowerCase();
      if (!q) {{
        nodes.forEach(function(n) {{ n.hidden = false; nodes.update(n); }});
        return;
      }}
      nodes.forEach(function(n) {{
        var match = n.label.toLowerCase().indexOf(q) !== -1 || n.sourceFile.toLowerCase().indexOf(q) !== -1;
        n.hidden = !match;
        nodes.update(n);
      }});
    }});
  </script>
</body>
</html>"##
    );

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out_path, html)?;
    Ok(())
}
