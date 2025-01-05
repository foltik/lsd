function markdownToHtml(md) {
  // Step 1: Apply markdown formatting rules
  let html = md
    // Block quotes: > text
    .replace(/^(?:> ?.*(?:\r?\n|$))+$/gm, (quote) => {
      const text = quote.replace(/^> ?/gm, "");
      return `<blockquote>${text.trim()}</blockquote>`;
    })
    // Unordered list: * text
    .replace(/^(?:\* .*(?:\r?\n|$))+$/gm, (block) => {
      // Split the block into individual lines
      const lines = block.trim().split(/\r?\n/);
      let listItems = "";
      for (const line of lines) {
        if (line.startsWith("* ")) {
          // Remove the leading '* ' and wrap in <li>
          const itemText = line.replace(/^\* +/, "").trim();
          if (itemText) {
            listItems += `<li>${itemText}</li>`;
          }
        }
      }
      return `<ul>${listItems}</ul>`;
    })
    // Bold: **text**
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    // Italic: *text*
    .replace(/\*(.+?)\*/g, "<em>$1</em>")
    // Headings: #, ##, ###, etc. starting at h2
    .replace(/^# (.*)$/gm, "<h2>$1</h2>")
    .replace(/^## (.*)$/gm, "<h3>$1</h3>")
    .replace(/^### (.*)$/gm, "<h4>$1</h4>")
    .replace(/^#### (.*)$/gm, "<h5>$1</h5>")
    .replace(/^##### (.*)$/gm, "<h6>$1</h6>")
    // Images: ![alt](url)
    .replace(/\!\[(.+)\]\((.*)\)/g, '<img alt="$1" src="$2" />')
    // Links: [text](url)
    .replace(/\[(.+)\]\((.*)\)/g, '<a href="$2">$1</a>');

  // Step 2: Split the text into paragraphs and wrap in <p>
  const paragraphs = html.trim().split(/\n\s*\n/);
  return paragraphs
    .map((p) => p.trim())
    .map((p) => {
      // If it's already a block-level element, ignore
      if (/^(<h\d|<ul|<pre|<blockquote)/.test(p)) {
        return p;
      }

      // Replace single newlines with <br>, then wrap in <p>
      const withBreaks = p.replace(/\n/g, "<br>");
      return `<p>${withBreaks}</p>`;
    })
    .join("\n");
}
