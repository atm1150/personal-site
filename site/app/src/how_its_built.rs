//! The "How it's built" article as a route, one component per section.

use crate::meta::{MetaDescription, MetaTitle, PageMeta, PageMetaTags};
use leptos::prelude::*;

#[component]
pub fn HowItsBuiltPage() -> impl IntoView {
    view! {
        <PageMetaTags meta=PageMeta {
            title: MetaTitle::new("How it's built - Andrew Miller")
                .expect("article title is a valid MetaTitle"),
            description: MetaDescription::new(
                "This article is about how the site you are viewing works: the frontend \
                 technologies used and some specific design choices.",
            )
            .expect("article description is a valid MetaDescription"),
            image: None,
        }/>
        <article class="prose">
            <h1>"How it's built"</h1>
            <p>
                "This article is about how the site you are viewing works. Specifically, this \
                is going to be talking more about the frontend: both the technologies used and \
                some specific design choices. The full "
                <a href="https://github.com/atm1150/personal-site">"source code"</a>
                " is on GitHub."
            </p>
            <WhyRust/>
            <hr/>
            <WhyLeptos/>
            <hr/>
            <ServerRenderingAndHydration/>
            <hr/>
            <VisualDesign/>
            <hr/>
            <Accessibility/>
        </article>
    }
}

#[component]
fn WhyRust() -> impl IntoView {
    view! {
        <section>
            <h2>"Why Rust"</h2>
            <p>
                "The first choice I was required to make for this site was the programming \
                language to be used, and in my case I decided to go with the Rust programming \
                language. There are multiple reasons to choose Rust, but I primarily chose it \
                because I simply wanted to use Rust. I have been experimenting with Rust in my \
                free time, which has been very enjoyable for me."
            </p>
            <h3>"Static typing all the way to the browser"</h3>
            <p>
                "Some people are probably wondering how Rust is even a choice to be made when it \
                comes to frontend web design, and the answer to that is WASM. WASM stands for \
                WebAssembly, and it is a compilation target that modern browsers execute in a \
                sandboxed environment with JavaScript bindings to browser APIs. This allows \
                frontend web design to be performed with any language that can target WASM, and \
                Rust can compile to WASM natively, and the wasm-bindgen crate creates the required \
                JavaScript bindings. Static typing for the frontend does not absolutely require \
                WASM: TypeScript does exist, but its types are erased at compile time, whereas \
                Rust/WASM types are inherently a part of the compiled program."
            </p>
            <RustVsBlazor/>
        </section>
    }
}

#[component]
fn RustVsBlazor() -> impl IntoView {
    view! {
        <h3>"Blazor, the familiar choice"</h3>
        <p>
            "Rust has appealed to me due to its expressive type system, compiler-enforced safety \
            during concurrency, and its async/await support that allows chaining method calls off \
            of awaits. People who have looked over my resume will notice a plethora of C# \
            experience that includes Blazor WASM and wonder why I didn't just choose to use what \
            I am already familiar with. Each of the above reasons is something that C#/Blazor \
            either does not have, or has only to a more limited extent. Blazor also requires a \
            download of the .NET runtime, whereas this Rust WASM payload is about 440 KB in total \
            and has no runtime at all, let alone a runtime as massive as .NET. This was measured \
            Sep 15th, 2026. Rust provides me with the same tools as C#, and it allows me to \
            create a professional example of my Rust skills. It is also a way to show off my \
            abilities as a polyglot developer. One advantage Blazor does have is that its \
            ecosystem is more developed, but this site is not complicated enough for that to \
            truly matter."
        </p>
    }
}

#[component]
fn WhyLeptos() -> impl IntoView {
    view! {
        <section>
            <h2>"Why Leptos"</h2>
            <p>
                "Now we get to the question of framework choice, and for this site I decided to \
                go with Leptos. Prior to working with Blazor, my frontend experience was mostly \
                with raw HTML/CSS/JS primarily delivered through MVC templates. After working with \
                Blazor in a professional capacity, I really came to appreciate the component-based \
                models that modern frontend frameworks seem to be converging towards. Leptos also \
                uses a component-based model, so I can continue with this style of programming."
            </p>
            <LeptosVsDioxus/>
        </section>
    }
}

#[component]
fn LeptosVsDioxus() -> impl IntoView {
    view! {
        <h3>"Dioxus, the up-and-comer"</h3>
        <p>
            "Dioxus is another frontend framework gaining steam in the Rust community, and I \
            wanted to go over why I chose Leptos over it. I primarily wanted to do Leptos as I \
            conceptually enjoy its fine-grained reactive model, which is similar to SolidJS and \
            based on Reactively: each signal is wired to the exact DOM node it affects, so when a \
            value changes only that node updates. Dioxus uses the VDOM style of rendering instead, \
            where a change re-renders the component into a VDOM and a diff decides which real \
            nodes to update. That is more than performant enough for a site like this, but I \
            wanted to get some experience with a different rendering style, and Blazor has \
            already given me experience with "
            <a href="https://github.com/dotnet/aspnetcore/discussions/48890">
                "a VDOM style of rendering"
            </a>
            ". The flip side of this is that this site does not have enough DOM churn for \
            fine-grained updates to be worthwhile, but again, I just wanted to use it. Dioxus \
            also does not have the multi-target deploy advantage over Leptos to as large a degree \
            as its marketing would imply. Using Leptos, I can still deploy to mobile through PWAs, \
            and I am able to target desktop through the use of Tauri. Currently, Dioxus desktop \
            is a webview using Wry, the Tauri project's webview library, so going through Tauri \
            gets me the same webview. In the future, I may want to revisit Dioxus when its WGPU \
            renderer named 'Blitz' is finished, but for now I am merely keeping tabs on the \
            project."
        </p>
    }
}

#[component]
fn ServerRenderingAndHydration() -> impl IntoView {
    view! {
        <section>
            <h2>"Server rendering and hydration"</h2>
            <p>
                "We are now at the part of the discussion where we don't talk so much about tech \
                choices and more about how the site is served and rendered. Similar to how I \
                chose Rust for the frontend, I also chose it for the backend. I may write more \
                about this later, but for now just know that I used tokio/axum for the backend \
                server."
            </p>
            <p>
                "When writing the code for this site, I made sure to design it such that the Rust \
                code base was split into three main sections: the server crate, a frontend crate, \
                and finally an app code crate. The frontend crate does not actually determine the \
                content that is rendered; that is in the app crate. What the frontend does is \
                simply hydrate the web page with the attached payload made up of my WASM-compiled \
                app logic. For those wondering, hydration in this context means that when the \
                WASM payload activates in the browser, it will go through the page and take over \
                everything that is already there rather than rendering the content a second time. \
                This separation of concerns allows me to also compile the app logic into the \
                server crate, as native code rather than WASM, so that an initial visitor to my \
                server doesn't simply receive a minimal HTML page with a WASM payload that creates \
                the content. Now the server itself is capable of rendering the content's HTML and \
                delivering it to the requesting device while also providing the WASM payload that \
                will hydrate after the download completes."
            </p>
            <SplitModelAdvantages/>
        </section>
    }
}

#[component]
fn SplitModelAdvantages() -> impl IntoView {
    view! {
        <p>"This split model has a few advantages:"</p>
        <ul>
            <li>"By having an initial server render, the time to first paint is minimized."</li>
            <li>
                "SEO is improved, as some crawlers are not advanced enough to execute WASM \
                payloads to see the first render."
            </li>
            <li>
                "We can engage in progressive enhancement of the site: the server itself is \
                capable of providing the page content, which means users who disable \
                JavaScript/WASM can still view it."
            </li>
        </ul>
    }
}

#[component]
fn VisualDesign() -> impl IntoView {
    view! {
        <section>
            <h2>"Visual design"</h2>
            <p>
                "Time to talk about some of the visual design choices. The design has three jobs: \
                follow the accessibility guidelines, convey the information, and not distract \
                from it. I ended up going with some neutral slate colors and a bronze accent. I \
                went with the bronze as I just figured it would look nice, and it carries the \
                structure: headings, links, and the header and footer bands. For the rest, I had \
                AI generate a few color schemes and brought them to my more visually artistic \
                friends to see which they preferred. The light/dark mode choice is determined by \
                the OS of the client device, and applied by CSS, so that on initial load there \
                will not be a brief white flash of light on the screen. The header stays at the \
                top of the page rather than following you as you scroll, so the article gets the \
                whole window. I am a classic STEM-oriented individual in that the initial artistic \
                creativity of a project tends to be an area where I am less capable, but I am more \
                than capable of implementing a design. This meant AI was incredibly useful in \
                helping me come up with a design I liked that reviewed well."
            </p>
            <h3>"The resume page"</h3>
            <p>
                "The resume page itself earns a small mention here. Clicking the download link \
                immediately gets you a PDF of the resume with my specifications. If you look at \
                the "
                <a href="https://github.com/atm1150/personal-site">"source code"</a>
                ", you'll also notice some print CSS that seems unrelated to said download \
                button. The reason for this is simple: when I want to create a new copy \
                of the resume, I use the print CSS to make the new PDF file. Then I put the PDF \
                artifact behind the download button to ensure all users get an immediate download \
                rather than seeing the browser print dialog open which takes a second step and \
                creates ways the user could end up with a different-looking PDF than I intended."
            </p>
        </section>
    }
}

#[component]
fn Accessibility() -> impl IntoView {
    view! {
        <section>
            <h2>"Accessibility"</h2>
            <p>
                "For accessibility, I wanted to go with WCAG where it applies. Whenever I change \
                the site's colors, I check that they still have an appropriate contrast ratio. I \
                have a current page marking to ensure the nav bar shows the current page, and I \
                use HTML landmarks over divs with specific CSS classes to help out screen readers. \
                I also added a skip link for ease of use."
            </p>
        </section>
    }
}
