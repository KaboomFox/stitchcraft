// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="index.html">Introduction</a></span></li><li class="chapter-item expanded "><li class="part-title">User guide</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/index.html"><strong aria-hidden="true">1.</strong> User guide</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/install.html"><strong aria-hidden="true">1.1.</strong> Install</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/tutorials/first-sew-out.html"><strong aria-hidden="true">1.2.</strong> Your first sew-out on a Brother</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/tutorials/svg-to-pes.html"><strong aria-hidden="true">1.3.</strong> Turn an SVG into a PES file</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/how-to/index.html"><strong aria-hidden="true">1.4.</strong> How-to guides</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/how-to/check-a-file.html"><strong aria-hidden="true">1.4.1.</strong> Check a machine file</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/how-to/convert-formats.html"><strong aria-hidden="true">1.4.2.</strong> Convert between formats</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/how-to/fit-the-hoop.html"><strong aria-hidden="true">1.4.3.</strong> Fit a design to the hoop</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/how-to/report-a-bug.html"><strong aria-hidden="true">1.4.4.</strong> Report a bug</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/stitches/index.html"><strong aria-hidden="true">1.5.</strong> Stitch types</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/stitches/running.html"><strong aria-hidden="true">1.5.1.</strong> Running stitch</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/stitches/locks.html"><strong aria-hidden="true">1.5.2.</strong> Lock stitches, trims and stops</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/explanation/embroidery-basics.html"><strong aria-hidden="true">1.6.</strong> Embroidery basics</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/reference/index.html"><strong aria-hidden="true">1.7.</strong> Reference</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/reference/cli.html"><strong aria-hidden="true">1.7.1.</strong> Command line</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/reference/profiles.html"><strong aria-hidden="true">1.7.2.</strong> Machine profiles</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/reference/formats.html"><strong aria-hidden="true">1.7.3.</strong> File formats</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/reference/test-sheets.html"><strong aria-hidden="true">1.7.4.</strong> Test sheets</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/reference/diagnostics.html"><strong aria-hidden="true">1.7.5.</strong> Diagnostic codes</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/reference/params.html"><strong aria-hidden="true">1.7.6.</strong> Parameters</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/reference/params/common.html"><strong aria-hidden="true">1.7.6.1.</strong> Common parameters</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/reference/params/stroke.html"><strong aria-hidden="true">1.7.6.2.</strong> Stroke</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/reference/params/running.html"><strong aria-hidden="true">1.7.6.3.</strong> Running stitch</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/reference/params/repeat.html"><strong aria-hidden="true">1.7.6.4.</strong> Repeats and bean stitch</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/reference/params/satin.html"><strong aria-hidden="true">1.7.6.5.</strong> Satin column</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/reference/params/fill.html"><strong aria-hidden="true">1.7.6.6.</strong> Fill</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/reference/params/tatami.html"><strong aria-hidden="true">1.7.6.7.</strong> Tatami fill</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/reference/glossary.html"><strong aria-hidden="true">1.7.7.</strong> Glossary</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="user/reference/compatibility.html"><strong aria-hidden="true">1.7.8.</strong> VectorCraft compatibility</a></span></li></ol></li></ol><li class="chapter-item expanded "><li class="part-title">Design</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/index.html"><strong aria-hidden="true">2.</strong> Design documents</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/tdd.html"><strong aria-hidden="true">2.1.</strong> Technical design document</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/architecture.html"><strong aria-hidden="true">2.2.</strong> Architecture</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/data-model.html"><strong aria-hidden="true">2.3.</strong> Data model</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/params.html"><strong aria-hidden="true">2.4.</strong> Parameter registry</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/diagnostics.html"><strong aria-hidden="true">2.5.</strong> Diagnostics</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/svg-input.html"><strong aria-hidden="true">2.6.</strong> SVG input</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/engine-pipeline.html"><strong aria-hidden="true">2.7.</strong> Engine pipeline</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/algorithms/index.html"><strong aria-hidden="true">2.8.</strong> Stitch generators</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/algorithms/strokes.html"><strong aria-hidden="true">2.8.1.</strong> Strokes</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/algorithms/satin.html"><strong aria-hidden="true">2.8.2.</strong> Satin</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/algorithms/offset.html"><strong aria-hidden="true">2.8.2.1.</strong> Offset curves</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/algorithms/fills.html"><strong aria-hidden="true">2.8.3.</strong> Fills</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/algorithms/locks.html"><strong aria-hidden="true">2.8.4.</strong> Lock stitches</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/formats.html"><strong aria-hidden="true">2.9.</strong> Machine formats</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/rendering.html"><strong aria-hidden="true">2.10.</strong> Rendering previews</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/vectorcraft-integration.html"><strong aria-hidden="true">2.11.</strong> VectorCraft integration</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/rfc-vectorcraft-abi-v2.html"><strong aria-hidden="true">2.12.</strong> RFC: VectorCraft plug-in ABI v2</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/compatibility-gate.html"><strong aria-hidden="true">2.13.</strong> VectorCraft compatibility gate</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/conformance.html"><strong aria-hidden="true">2.14.</strong> Conformance testing</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/determinism.html"><strong aria-hidden="true">2.15.</strong> Determinism</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/guardrails.html"><strong aria-hidden="true">2.16.</strong> Guardrails</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/docs-pipeline.html"><strong aria-hidden="true">2.17.</strong> Documentation pipeline</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/inkstitch-compat-contract.html"><strong aria-hidden="true">2.18.</strong> Ink/Stitch compatibility contract</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/adr/index.html"><strong aria-hidden="true">2.19.</strong> Decision records</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/adr/0001-license-and-clean-room.html"><strong aria-hidden="true">2.19.1.</strong> 0001 Licence and clean room</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/adr/0002-host-agnostic-engine-plugin-first.html"><strong aria-hidden="true">2.19.2.</strong> 0002 Host-agnostic engine, plug-in first</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/adr/0003-parameter-registry.html"><strong aria-hidden="true">2.19.3.</strong> 0003 Parameter registry</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/adr/0004-determinism.html"><strong aria-hidden="true">2.19.4.</strong> 0004 Determinism</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/adr/0005-geometry-stack.html"><strong aria-hidden="true">2.19.5.</strong> 0005 Geometry stack</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/adr/0006-docs-mdbook-diataxis.html"><strong aria-hidden="true">2.19.6.</strong> 0006 Docs: mdBook and Diátaxis</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/adr/0007-pes-first-brother-profile.html"><strong aria-hidden="true">2.19.7.</strong> 0007 PES first, Brother profile</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/adr/0008-conformance-first.html"><strong aria-hidden="true">2.19.8.</strong> 0008 Conformance first</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/adr/0009-adopt-vectorcraft-conventions.html"><strong aria-hidden="true">2.19.9.</strong> 0009 VectorCraft conventions</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/adr/0010-diagnostics-with-codes.html"><strong aria-hidden="true">2.19.10.</strong> 0010 Coded diagnostics</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/adr/0011-movable-into-vectorcraft.html"><strong aria-hidden="true">2.19.11.</strong> 0011 Movable into VectorCraft</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/adr/0012-read-dont-copy.html"><strong aria-hidden="true">2.19.12.</strong> 0012 Read, never copy</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/adr/0013-brother-pe800-reference-machine.html"><strong aria-hidden="true">2.19.13.</strong> 0013 Brother PE800</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="design/adr/0014-generators-see-their-neighbours.html"><strong aria-hidden="true">2.19.14.</strong> 0014 Generators see their neighbours</a></span></li></ol></li></ol><li class="chapter-item expanded "><li class="part-title">Plan</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="plan/roadmap.html"><strong aria-hidden="true">3.</strong> Roadmap</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="plan/machine-testing.html"><strong aria-hidden="true">4.</strong> Machine testing</a></span></li><li class="chapter-item expanded "><li class="part-title">Contributing</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="contributing/index.html"><strong aria-hidden="true">5.</strong> Contributing</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="contributing/playbook-new-stitch-type.html"><strong aria-hidden="true">5.1.</strong> New stitch type</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="contributing/playbook-new-param.html"><strong aria-hidden="true">5.2.</strong> New parameter</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="contributing/playbook-new-format.html"><strong aria-hidden="true">5.3.</strong> New format</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="contributing/playbook-new-diagnostic.html"><strong aria-hidden="true">5.4.</strong> New diagnostic</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="contributing/review-checklist.html"><strong aria-hidden="true">5.5.</strong> Review checklist</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="contributing/writing-style.html"><strong aria-hidden="true">5.6.</strong> Writing style</a></span></li></ol></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString().split('#')[0].split('?')[0];
        if (current_page.endsWith('/')) {
            current_page += 'index.html';
        }
        const links = Array.prototype.slice.call(this.querySelectorAll('a'));
        const l = links.length;
        for (let i = 0; i < l; ++i) {
            const link = links[i];
            const href = link.getAttribute('href');
            if (href && !href.startsWith('#') && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The 'index' page is supposed to alias the first chapter in the book.
            // Check both with and without the '.html' suffix to be robust against pretty URLs
            if (link.href.replace(/\.html$/, '') === current_page.replace(/\.html$/, '')
                || i === 0
                && path_to_root === ''
                && current_page.endsWith('/index.html')) {
                link.classList.add('active');
                let parent = link.parentElement;
                while (parent) {
                    if (parent.tagName === 'LI' && parent.classList.contains('chapter-item')) {
                        parent.classList.add('expanded');
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', e => {
            if (e.target.tagName === 'A') {
                const clientRect = e.target.getBoundingClientRect();
                const sidebarRect = this.getBoundingClientRect();
                sessionStorage.setItem('sidebar-scroll-offset', clientRect.top - sidebarRect.top);
            }
        }, { passive: true });
        const sidebarScrollOffset = sessionStorage.getItem('sidebar-scroll-offset');
        sessionStorage.removeItem('sidebar-scroll-offset');
        if (sidebarScrollOffset !== null) {
            // preserve sidebar scroll position when navigating via links within sidebar
            const activeSection = this.querySelector('.active');
            if (activeSection) {
                const clientRect = activeSection.getBoundingClientRect();
                const sidebarRect = this.getBoundingClientRect();
                const currentOffset = clientRect.top - sidebarRect.top;
                this.scrollTop += currentOffset - parseFloat(sidebarScrollOffset);
            }
        } else {
            // scroll sidebar to current active section when navigating via
            // 'next/previous chapter' buttons
            const activeSection = document.querySelector('#mdbook-sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        const sidebarAnchorToggles = document.querySelectorAll('.chapter-fold-toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(el => {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define('mdbook-sidebar-scrollbox', MDBookSidebarScrollbox);


// ---------------------------------------------------------------------------
// Support for dynamically adding headers to the sidebar.

(function() {
    // This is used to detect which direction the page has scrolled since the
    // last scroll event.
    let lastKnownScrollPosition = 0;
    // This is the threshold in px from the top of the screen where it will
    // consider a header the "current" header when scrolling down.
    const defaultDownThreshold = 150;
    // Same as defaultDownThreshold, except when scrolling up.
    const defaultUpThreshold = 300;
    // The threshold is a virtual horizontal line on the screen where it
    // considers the "current" header to be above the line. The threshold is
    // modified dynamically to handle headers that are near the bottom of the
    // screen, and to slightly offset the behavior when scrolling up vs down.
    let threshold = defaultDownThreshold;
    // This is used to disable updates while scrolling. This is needed when
    // clicking the header in the sidebar, which triggers a scroll event. It
    // is somewhat finicky to detect when the scroll has finished, so this
    // uses a relatively dumb system of disabling scroll updates for a short
    // time after the click.
    let disableScroll = false;
    // Array of header elements on the page.
    let headers;
    // Array of li elements that are initially collapsed headers in the sidebar.
    // I'm not sure why eslint seems to have a false positive here.
    // eslint-disable-next-line prefer-const
    let headerToggles = [];
    // This is a debugging tool for the threshold which you can enable in the console.
    let thresholdDebug = false;

    // Updates the threshold based on the scroll position.
    function updateThreshold() {
        const scrollTop = window.pageYOffset || document.documentElement.scrollTop;
        const windowHeight = window.innerHeight;
        const documentHeight = document.documentElement.scrollHeight;

        // The number of pixels below the viewport, at most documentHeight.
        // This is used to push the threshold down to the bottom of the page
        // as the user scrolls towards the bottom.
        const pixelsBelow = Math.max(0, documentHeight - (scrollTop + windowHeight));
        // The number of pixels above the viewport, at least defaultDownThreshold.
        // Similar to pixelsBelow, this is used to push the threshold back towards
        // the top when reaching the top of the page.
        const pixelsAbove = Math.max(0, defaultDownThreshold - scrollTop);
        // How much the threshold should be offset once it gets close to the
        // bottom of the page.
        const bottomAdd = Math.max(0, windowHeight - pixelsBelow - defaultDownThreshold);
        let adjustedBottomAdd = bottomAdd;

        // Adjusts bottomAdd for a small document. The calculation above
        // assumes the document is at least twice the windowheight in size. If
        // it is less than that, then bottomAdd needs to be shrunk
        // proportional to the difference in size.
        if (documentHeight < windowHeight * 2) {
            const maxPixelsBelow = documentHeight - windowHeight;
            const t = 1 - pixelsBelow / Math.max(1, maxPixelsBelow);
            const clamp = Math.max(0, Math.min(1, t));
            adjustedBottomAdd *= clamp;
        }

        let scrollingDown = true;
        if (scrollTop < lastKnownScrollPosition) {
            scrollingDown = false;
        }

        if (scrollingDown) {
            // When scrolling down, move the threshold up towards the default
            // downwards threshold position. If near the bottom of the page,
            // adjustedBottomAdd will offset the threshold towards the bottom
            // of the page.
            const amountScrolledDown = scrollTop - lastKnownScrollPosition;
            const adjustedDefault = defaultDownThreshold + adjustedBottomAdd;
            threshold = Math.max(adjustedDefault, threshold - amountScrolledDown);
        } else {
            // When scrolling up, move the threshold down towards the default
            // upwards threshold position. If near the bottom of the page,
            // quickly transition the threshold back up where it normally
            // belongs.
            const amountScrolledUp = lastKnownScrollPosition - scrollTop;
            const adjustedDefault = defaultUpThreshold - pixelsAbove
                + Math.max(0, adjustedBottomAdd - defaultDownThreshold);
            threshold = Math.min(adjustedDefault, threshold + amountScrolledUp);
        }

        if (documentHeight <= windowHeight) {
            threshold = 0;
        }

        if (thresholdDebug) {
            const id = 'mdbook-threshold-debug-data';
            let data = document.getElementById(id);
            if (data === null) {
                data = document.createElement('div');
                data.id = id;
                data.style.cssText = `
                    position: fixed;
                    top: 50px;
                    right: 10px;
                    background-color: 0xeeeeee;
                    z-index: 9999;
                    pointer-events: none;
                `;
                document.body.appendChild(data);
            }
            data.innerHTML = `
                <table>
                  <tr><td>documentHeight</td><td>${documentHeight.toFixed(1)}</td></tr>
                  <tr><td>windowHeight</td><td>${windowHeight.toFixed(1)}</td></tr>
                  <tr><td>scrollTop</td><td>${scrollTop.toFixed(1)}</td></tr>
                  <tr><td>pixelsAbove</td><td>${pixelsAbove.toFixed(1)}</td></tr>
                  <tr><td>pixelsBelow</td><td>${pixelsBelow.toFixed(1)}</td></tr>
                  <tr><td>bottomAdd</td><td>${bottomAdd.toFixed(1)}</td></tr>
                  <tr><td>adjustedBottomAdd</td><td>${adjustedBottomAdd.toFixed(1)}</td></tr>
                  <tr><td>scrollingDown</td><td>${scrollingDown}</td></tr>
                  <tr><td>threshold</td><td>${threshold.toFixed(1)}</td></tr>
                </table>
            `;
            drawDebugLine();
        }

        lastKnownScrollPosition = scrollTop;
    }

    function drawDebugLine() {
        if (!document.body) {
            return;
        }
        const id = 'mdbook-threshold-debug-line';
        const existingLine = document.getElementById(id);
        if (existingLine) {
            existingLine.remove();
        }
        const line = document.createElement('div');
        line.id = id;
        line.style.cssText = `
            position: fixed;
            top: ${threshold}px;
            left: 0;
            width: 100vw;
            height: 2px;
            background-color: red;
            z-index: 9999;
            pointer-events: none;
        `;
        document.body.appendChild(line);
    }

    function mdbookEnableThresholdDebug() {
        thresholdDebug = true;
        updateThreshold();
        drawDebugLine();
    }

    window.mdbookEnableThresholdDebug = mdbookEnableThresholdDebug;

    // Updates which headers in the sidebar should be expanded. If the current
    // header is inside a collapsed group, then it, and all its parents should
    // be expanded.
    function updateHeaderExpanded(currentA) {
        // Add expanded to all header-item li ancestors.
        let current = currentA.parentElement;
        while (current) {
            if (current.tagName === 'LI' && current.classList.contains('header-item')) {
                current.classList.add('expanded');
            }
            current = current.parentElement;
        }
    }

    // Updates which header is marked as the "current" header in the sidebar.
    // This is done with a virtual Y threshold, where headers at or below
    // that line will be considered the current one.
    function updateCurrentHeader() {
        if (!headers || !headers.length) {
            return;
        }

        // Reset the classes, which will be rebuilt below.
        const els = document.getElementsByClassName('current-header');
        for (const el of els) {
            el.classList.remove('current-header');
        }
        for (const toggle of headerToggles) {
            toggle.classList.remove('expanded');
        }

        // Find the last header that is above the threshold.
        let lastHeader = null;
        for (const header of headers) {
            const rect = header.getBoundingClientRect();
            if (rect.top <= threshold) {
                lastHeader = header;
            } else {
                break;
            }
        }
        if (lastHeader === null) {
            lastHeader = headers[0];
            const rect = lastHeader.getBoundingClientRect();
            const windowHeight = window.innerHeight;
            if (rect.top >= windowHeight) {
                return;
            }
        }

        // Get the anchor in the summary.
        const href = '#' + lastHeader.id;
        const a = [...document.querySelectorAll('.header-in-summary')]
            .find(element => element.getAttribute('href') === href);
        if (!a) {
            return;
        }

        a.classList.add('current-header');

        updateHeaderExpanded(a);
    }

    // Updates which header is "current" based on the threshold line.
    function reloadCurrentHeader() {
        if (disableScroll) {
            return;
        }
        updateThreshold();
        updateCurrentHeader();
    }


    // When clicking on a header in the sidebar, this adjusts the threshold so
    // that it is located next to the header. This is so that header becomes
    // "current".
    function headerThresholdClick(event) {
        // See disableScroll description why this is done.
        disableScroll = true;
        setTimeout(() => {
            disableScroll = false;
        }, 100);
        // requestAnimationFrame is used to delay the update of the "current"
        // header until after the scroll is done, and the header is in the new
        // position.
        requestAnimationFrame(() => {
            requestAnimationFrame(() => {
                // Closest is needed because if it has child elements like <code>.
                const a = event.target.closest('a');
                const href = a.getAttribute('href');
                const targetId = href.substring(1);
                const targetElement = document.getElementById(targetId);
                if (targetElement) {
                    threshold = targetElement.getBoundingClientRect().bottom;
                    updateCurrentHeader();
                }
            });
        });
    }

    // Takes the nodes from the given head and copies them over to the
    // destination, along with some filtering.
    function filterHeader(source, dest) {
        const clone = source.cloneNode(true);
        clone.querySelectorAll('mark').forEach(mark => {
            mark.replaceWith(...mark.childNodes);
        });
        dest.append(...clone.childNodes);
    }

    // Scans page for headers and adds them to the sidebar.
    document.addEventListener('DOMContentLoaded', function() {
        const activeSection = document.querySelector('#mdbook-sidebar .active');
        if (activeSection === null) {
            return;
        }

        const main = document.getElementsByTagName('main')[0];
        headers = Array.from(main.querySelectorAll('h2, h3, h4, h5, h6'))
            .filter(h => h.id !== '' && h.children.length && h.children[0].tagName === 'A');

        if (headers.length === 0) {
            return;
        }

        // Build a tree of headers in the sidebar.

        const stack = [];

        const firstLevel = parseInt(headers[0].tagName.charAt(1));
        for (let i = 1; i < firstLevel; i++) {
            const ol = document.createElement('ol');
            ol.classList.add('section');
            if (stack.length > 0) {
                stack[stack.length - 1].ol.appendChild(ol);
            }
            stack.push({level: i + 1, ol: ol});
        }

        // The level where it will start folding deeply nested headers.
        const foldLevel = 3;

        for (let i = 0; i < headers.length; i++) {
            const header = headers[i];
            const level = parseInt(header.tagName.charAt(1));

            const currentLevel = stack[stack.length - 1].level;
            if (level > currentLevel) {
                // Begin nesting to this level.
                for (let nextLevel = currentLevel + 1; nextLevel <= level; nextLevel++) {
                    const ol = document.createElement('ol');
                    ol.classList.add('section');
                    const last = stack[stack.length - 1];
                    const lastChild = last.ol.lastChild;
                    // Handle the case where jumping more than one nesting
                    // level, which doesn't have a list item to place this new
                    // list inside of.
                    if (lastChild) {
                        lastChild.appendChild(ol);
                    } else {
                        last.ol.appendChild(ol);
                    }
                    stack.push({level: nextLevel, ol: ol});
                }
            } else if (level < currentLevel) {
                while (stack.length > 1 && stack[stack.length - 1].level > level) {
                    stack.pop();
                }
            }

            const li = document.createElement('li');
            li.classList.add('header-item');
            li.classList.add('expanded');
            if (level < foldLevel) {
                li.classList.add('expanded');
            }
            const span = document.createElement('span');
            span.classList.add('chapter-link-wrapper');
            const a = document.createElement('a');
            span.appendChild(a);
            a.href = '#' + header.id;
            a.classList.add('header-in-summary');
            filterHeader(header.children[0], a);
            a.addEventListener('click', headerThresholdClick);
            const nextHeader = headers[i + 1];
            if (nextHeader !== undefined) {
                const nextLevel = parseInt(nextHeader.tagName.charAt(1));
                if (nextLevel > level && level >= foldLevel) {
                    const toggle = document.createElement('a');
                    toggle.classList.add('chapter-fold-toggle');
                    toggle.classList.add('header-toggle');
                    toggle.addEventListener('click', () => {
                        li.classList.toggle('expanded');
                    });
                    const toggleDiv = document.createElement('div');
                    toggleDiv.textContent = '❱';
                    toggle.appendChild(toggleDiv);
                    span.appendChild(toggle);
                    headerToggles.push(li);
                }
            }
            li.appendChild(span);

            const currentParent = stack[stack.length - 1];
            currentParent.ol.appendChild(li);
        }

        const onThisPage = document.createElement('div');
        onThisPage.classList.add('on-this-page');
        onThisPage.append(stack[0].ol);
        const activeItemSpan = activeSection.parentElement;
        activeItemSpan.after(onThisPage);
    });

    document.addEventListener('DOMContentLoaded', reloadCurrentHeader);
    document.addEventListener('scroll', reloadCurrentHeader, { passive: true });
})();

