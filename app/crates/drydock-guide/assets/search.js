(function () {
  function setup() {
    var records = window.DRYDOCK_SEARCH || [];
    var input = document.getElementById("search");
    var results = document.getElementById("search-results");
    if (!input || !results) {
      return;
    }

    function clear() {
      while (results.firstChild) {
        results.removeChild(results.firstChild);
      }
    }

    function search() {
      clear();
      var words = input.value.toLowerCase().split(/\s+/).filter(function (word) {
        return word.length > 0;
      });
      if (words.length === 0) {
        return;
      }

      var matches = [];
      records.forEach(function (record) {
        var heading = record.heading.toLowerCase();
        var text = record.text.toLowerCase();
        var all = words.every(function (word) {
          return heading.indexOf(word) >= 0 || text.indexOf(word) >= 0;
        });
        if (all) {
          var inHeading = words.every(function (word) {
            return heading.indexOf(word) >= 0;
          });
          matches.push({ record: record, inHeading: inHeading });
        }
      });

      matches.sort(function (a, b) {
        return (b.inHeading ? 1 : 0) - (a.inHeading ? 1 : 0);
      });

      matches.slice(0, 20).forEach(function (match) {
        var item = document.createElement("li");
        var link = document.createElement("a");
        link.href = document.body.dataset.root + match.record.url;
        link.textContent = match.record.heading + " \u2014 " + match.record.title;
        item.appendChild(link);
        results.appendChild(item);
      });
    }

    input.addEventListener("input", search);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", setup);
  } else {
    setup();
  }
})();
