use crate::meta::{MetaDescription, MetaTitle, PageMeta, PageMetaTags};
use leptos::prelude::*;
use singlestage::Card;

/// The resume as a first-class app route.
#[component]
pub fn ResumePage() -> impl IntoView {
    view! {
        <PageMetaTags meta=PageMeta {
            title: MetaTitle::new("Andrew Miller — Software Engineer")
                .expect("resume title is a valid MetaTitle"),
            description: MetaDescription::new(
                "Resume of Andrew Miller, software engineer in Seattle, WA.",
            )
            .expect("resume description is a valid MetaDescription"),
            image: None,
        }/>
        <div class="resume-page">
            <Card class="resume-card">
                // sticky download link
                <a class="no-print" href="/Andrew-Miller-Resume.pdf" download="" rel="external">
                    "Download PDF"
                </a>
                <div class="page">
                    <header class="masthead">
                        <h1>"Andrew Miller"</h1>
                        <p class="role">".NET Backend Software Engineer"</p>
                        <p class="contact">
                            "Seattle, WA"
                            <span class="sep">"·"</span>
                            "(812) 459-7361"
                            <span class="sep">"·"</span>
                            <a href="mailto:atmiller1150@gmail.com">"atmiller1150@gmail.com"</a>
                            <span class="sep">"·"</span>
                            <a href="https://www.linkedin.com/in/atmiller1150/">"linkedin.com/in/atmiller1150"</a>
                        </p>
                    </header>

                    <div class="resume-body">
                        <section id="summary">
                            <h2>"Summary"</h2>
                            <p>
                                "Backend software engineer with approximately 4.5 years of professional C#/.NET experience, focused on
                                modernizing legacy systems. For the past 3.5 years, served as the primary backend developer for a
                                compliance-intensive mortgage document generation platform where precision is critical.
                                Experiments with Rust outside of work."
                            </p>
                        </section>

                        <section id="skills">
                            <h2>"Skills"</h2>
                            <dl class="skills">
                                <div class="skill-row">
                                    <dt>"Languages"</dt>
                                    <dd>"C#, Rust, SQL/T-SQL, JavaScript, HTML/CSS"</dd>
                                </div>
                                <div class="skill-row">
                                    <dt>".NET & Web"</dt>
                                    <dd>"ASP.NET Core, .NET 8-10, Blazor (Server & WebAssembly), Entity Framework Core, Dapper, MediatR/CQRS, REST/GraphQL APIs, OpenAPI/Swagger, Postman, .NET Aspire"</dd>
                                </div>
                                <div class="skill-row">
                                    <dt>"Data & Messaging"</dt>
                                    <dd>"SQL Server/Azure SQL, Redis, RabbitMQ (MassTransit), event-driven architecture"</dd>
                                </div>
                                <div class="skill-row">
                                    <dt>"Cloud & DevOps"</dt>
                                    <dd>"Azure, GitHub Actions, CI/CD, containers (Docker, Podman), Git"</dd>
                                </div>
                                <div class="skill-row">
                                    <dt>"Testing"</dt>
                                    <dd>"Unit (xUnit/bUnit) & end-to-end (Playwright) testing"</dd>
                                </div>
                                <div class="skill-row">
                                    <dt>"AI"</dt>
                                    <dd>"LLM feature integration (Gemini), agentic development workflows (Claude Code)"</dd>
                                </div>
                                <div class="skill-row">
                                    <dt>"Mortgage Domain"</dt>
                                    <dd>"Encompass (ICE Mortgage) LOS integration, MISMO/UCD 2 XML, TRID (Loan Estimate & Closing Disclosure), RESPA, compliant document generation (Aspose)"</dd>
                                </div>
                            </dl>
                        </section>

                        <section id="experience">
                            <h2>"Experience"</h2>

                            <article class="job">
                                <div class="job-head">
                                    <h3>"PPDocs " <span class="job-title">"- Software Developer, C#/.NET Backend"</span></h3>
                                    <span class="dates">"Jan 2023 - Jun 2026"</span>
                                </div>
                                <p class="job-sub">"Mortgage loan document-generation platform serving lenders in all 50 states"</p>
                                <ul>
                                    <li>"Modernized a 20-year-old VB.NET codebase to .NET 8-10, migrating business logic from legacy stored procedures into MediatR/CQRS handler pipelines using Entity Framework Core and Dapper."</li>
                                    <li>"Built a multithreaded C# tool that parsed a 50+ page legacy XHTML/JavaScript order form governed by state-specific legal, internal, and monetary rules, then generated equivalent Blazor Server pages with visibility conditions, save/retrieval wiring, and backing C# behavior objects, automating the bulk of the UI migration."</li>
                                    <li>"Designed and implemented the team's .NET Aspire setup end to end, orchestrating five services plus Azure SQL, RabbitMQ, Redis, and MailHog under one local startup, with a test-database builder for reproducible resets."</li>
                                    <li>"Wrote a custom Azure SQL Aspire integration to keep local development from falling back to the default SQL Server (MSSQL) container, which cannot run on the team's ARM-based laptops."</li>
                                    <li>"Built event-driven workflows that route newly submitted loans from the order form UI or API to the intranet whiteboard for attorney review."</li>
                                    <li>"Built mortgage document-generation features, assembling C#-computed content and tables into Aspose Word templates (rendered to PDF) - a library of 100+ compliant loan documents spanning TRID, RESPA, and non-RESPA packages."</li>
                                    <li>"Built an LLM-powered mapping feature using Gemini Flash to match customers' free-text interest-rate source descriptions to platform index definitions and present the suggested match for confirmation, automating a manual, error-prone step."</li>
                                    <li>"Built the Encompass Partner Connect integration for ICE Mortgage Technology and implemented UCD 2 delivery in MISMO XML to clients or directly to the GSEs, including Fannie Mae and Freddie Mac."</li>
                                </ul>
                            </article>

                            <article class="job">
                                <div class="job-head">
                                    <h3>"Aeravida " <span class="job-title">"- Software Developer, .NET"</span></h3>
                                    <span class="dates">"Jan 2022 - Dec 2022"</span>
                                </div>
                                <p class="job-sub">"E-commerce platform for handmade and fair-trade goods"</p>
                                <ul>
                                    <li>"Helped rebuild a legacy .NET e-commerce application on ASP.NET MVC while maintaining and extending the live production site."</li>
                                    <li>"Integrated Wayfair's GraphQL and Walmart's REST seller APIs to publish and sync the product catalog, opening two additional marketplace sales channels."</li>
                                    <li>"Recruited after a year by the company's co-owner - PPDocs' system architect - to join PPDocs as a backend developer."</li>
                                </ul>
                            </article>

                            <article class="job">
                                <div class="job-head">
                                    <h3>"Silgan Closures " <span class="job-title">"- Operator to Mechanic (prior to software career)"</span></h3>
                                    <span class="dates">"May 2014 - Jan 2022"</span>
                                </div>
                                <ul>
                                    <li>"Progressed from machine operator to mechanic, diagnosing and repairing faults on production machinery."</li>
                                    <li>"Performed troubleshooting and quality-assurance checks at both levels - the same systematic diagnose-isolate-fix loop later applied to software debugging."</li>
                                </ul>
                            </article>
                        </section>

                        <section id="education">
                            <h2>"Education & Training"</h2>
                            <div class="edu-row">
                                <span class="edu-name">"The Software Guild " <span class="edu-detail">"- C#/.NET Full-Stack Developer Certification"</span></span>
                                <span class="dates">"2020 - 2021"</span>
                            </div>
                            <div class="edu-row">
                                <span class="edu-name">"University of Evansville " <span class="edu-detail">"- Coursework in Mathematics, Physics & Accounting"</span></span>
                                <span class="dates">"2010 - 2014"</span>
                            </div>
                        </section>
                    </div>
                </div>
            </Card>
        </div>
    }
}
