(function() {
    var PROTOCOL = window.location.protocol;
    var HOSTNAME = window.location.hostname;
    var PORT = window.location.port;
    var PATHNAME = window.location.pathname;
    
    // Attempt to determine the base URL of the repository
    // Assumption: The site is hosted at root or a subdirectory (e.g. /rxRust/)
    // We look for versions.json at the parent level of the current version folder
    // E.g. /rxRust/latest/index.html -> /rxRust/versions.json
    
    // Heuristic: traverse up until we find versions.json or hit root
    // But simplest for GitHub Pages: The root of the site.
    // If we are at /rxRust/latest/foo/bar.html, we need /rxRust/versions.json
    
    // Let's try to deduce the root path.
    // If the path is /rxRust/v1.0.0/..., the root is /rxRust/
    
    // We will use a relative fetch "../versions.json" or "../../versions.json" approach 
    // is risky because depth varies.
    // Instead, let's use the absolute path if we know the repo name, or try to guess.
    
    // For now, we will assume the structure is always /{version}/... 
    // So versions.json is always at /rxRust/versions.json or /versions.json.
    
    // Let's try to fetch from the site root first? 
    // Actually, usually the path is /{repo-name}/{version}/...
    
    // Let's try to find the "root" by looking at the known deployment structure.
    // We can rely on the fact that we are in a version folder.
    
    const CURRENT_VERSION_MATCH = window.location.pathname.match(/\/([^\/]+?)\//g); 
    // This is tricky. Let's try to fetch versions.json from likely locations.
    
    const potentialPaths = [
        '../versions.json',
        '../../versions.json',
        '/rxRust/versions.json',
        '/versions.json'
    ];
    
    function initVersionPicker(versions) {
        var sidebar = document.querySelector('.sidebar');
        if (!sidebar) return;

        // Create container
        var container = document.createElement('div');
        container.style.padding = '10px';
        container.style.textAlign = 'center';
        container.className = 'version-picker-container';

        // Label
        var label = document.createElement('div');
        label.textContent = 'Version:';
        label.style.fontWeight = 'bold';
        label.style.marginBottom = '5px';
        container.appendChild(label);

        // Select
        var select = document.createElement('select');
        select.style.width = '100%';
        select.style.padding = '5px';
        select.style.borderRadius = '5px';
        
        // Find current version from URL
        // We assume the URL contains the version string
        var currentPath = window.location.pathname;
        var currentVersion = versions.find(v => currentPath.includes('/' + v.path + '/')) || versions[0];

        versions.forEach(function(v) {
            var option = document.createElement('option');
            option.value = v.path;
            option.textContent = v.name;
            if (currentVersion && v.path === currentVersion.path) {
                option.selected = true;
            }
            select.appendChild(option);
        });

        select.addEventListener('change', function(e) {
            var targetVersion = e.target.value;
            // Construct new URL
            // Replace the current version segment with the new one
            // This is naive replacement; if structure changes, it breaks.
            // Robust way: Go to the root of that version.
            
            // Assume base structure: /rxRust/{version}/...
            // We want to redirect to /rxRust/{targetVersion}/index.html usually,
            // or try to keep the current page if it exists.
            
            if (currentVersion) {
                 var newPath = currentPath.replace('/' + currentVersion.path + '/', '/' + targetVersion + '/');
                 window.location.href = newPath;
            } else {
                // Fallback
                // Try to find where the version starts.
                // If we can't guess, just go to the root of that version
                // But we don't know the repo root easily without config.
                // Let's assume standard GH pages structure: location.origin + /rxRust/ + version
                 window.location.href = window.location.origin + window.location.pathname.split('/').slice(0, 2).join('/') + '/' + targetVersion + '/';
            }
        });

        container.appendChild(select);

        // Insert at the top of the sidebar
        sidebar.insertBefore(container, sidebar.firstChild);
    }

    // Try to fetch versions.json
    function fetchVersions(paths) {
        if (paths.length === 0) return;
        var path = paths.shift();
        
        fetch(path)
            .then(response => {
                if (!response.ok) throw new Error("404");
                return response.json();
            })
            .then(data => {
                // Validate data
                if (Array.isArray(data)) {
                    initVersionPicker(data);
                }
            })
            .catch(err => {
                fetchVersions(paths);
            });
    }

    fetchVersions(potentialPaths);

})();
