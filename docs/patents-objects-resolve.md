# Freedom-to-operate (technical reading): `objects.resolve` vs. Definiens/Trimble patents

Date of research: 2026-11-19. **This is a technical analysis by a non-lawyer, not legal advice.** It reads claim text against a description of what `objects.resolve` does. Claim construction, validity, term calculation and infringement are legal questions; section 6 lists what a patent attorney should confirm.

Sources: claim text was taken verbatim from the Google Patents HTML (claims section) and checked against the front pages of the official USPTO PDFs that Google Patents serves. Status data comes from the USPTO legal events that Google Patents mirrors. **Could not access:** USPTO Patent Center (the file wrapper loads only through JavaScript, so I could not see which patent the terminal disclaimer names), the USPTO Assignment API (DNS failure), Espacenet and INPI (not reachable from here), Justia (403). Google Patents began rate-limiting (HTTP 503/"Sorry") partway through, so three possibly relevant patents (US9740957B2, US9805248B2, US10474874B2) could not be read.

---

## 1. The two known patents

### 1.1 US 8,319,793 B2: "Analyzing pixel data by imprinting objects of a computer-implemented network structure into other objects"

| Item | Value | Source |
|---|---|---|
| Application | US 12/386,380, filed 2009-04-17; published as US 2010/0265267 A1 | [Google Patents](https://patents.google.com/patent/US8319793B2/en) |
| Priority | 2009-04-17 (no earlier US or foreign priority claimed) | same |
| Grant | 2012-11-27 | same |
| Inventors | Arno Schaepe, Guenter Schmidt, Owen Feehan, Gerd Binnig | same |
| Assignee of record | Definiens AG, Munich (assignment recorded 2009, reel/frame 022613/0799). No later assignment to Trimble appears in the mirrored events. | same |
| PTA | **894 days** (front page notice) | [USPTO PDF](https://patentimages.storage.googleapis.com/1a/95/b2/f54eee99659eb4/US8319793.pdf) |
| **Terminal disclaimer** | Front page: "This patent is subject to a terminal disclaimer" (the issue date carries an asterisk) | same PDF |
| Maintenance fees | 4th year (2016-05-23), 8th year (2020-05-22) and 12th year (2024-05-15) all paid, large entity | Google Patents legal events |
| Google "adjusted expiration" | 2031-09-28 (= 2009-04-17 + 20 years + 894 days PTA) | Google Patents |
| References cited | Only US 7,801,361 B2 and its publication US 2008/0008349 A1, both cited by the examiner | USPTO PDF front page |
| Family | **US only.** No EP, DE, WO, BR or JP members. | Google Patents "Worldwide applications" |

**Important correction to the plan's expiry date.** Google's 2031-09-28 adds the PTA and ignores the terminal disclaimer. Under 35 U.S.C. 154(b)(2)(B), "[n]o patent the term of which has been disclaimed beyond a specified date may be adjusted under this section beyond the expiration date specified in the disclaimer" ([MPEP 1490](https://www.uspto.gov/web/offices/pac/mpep/s1490.html); *In re Cellect*, 81 F.4th 1216 (Fed. Cir. 2023)). I could not read the disclaimer itself (Patent Center). The only art the examiner cited was US 7,801,361, which has the same assignee, overlapping inventors (Binnig, Schmidt) and the same attorney (Imperium Patent Works). So the disclaimer is most likely over **US 7,801,361**. In that case US 8,319,793 expires **no later than US 7,801,361, about 2029-01-20**, not 2031-09-28. The standard disclaimer wording in MPEP 1490 also makes the patent "enforceable only for and during such period that it and the reference patent are commonly owned". *To confirm:* which patent is named in the disclaimer, and whether its date includes the reference patent's PTA. If the disclaimer named US 7,873,223 instead, the date would be earlier still (see 1.3).

**Independent claims** (verbatim from [Google Patents](https://patents.google.com/patent/US8319793B2/en)):

- **Claim 1.** A method comprising: (a) specifying a first object class; (b) specifying a second object class; (c) generating a first object network, wherein each object of the first object network belongs to the first object class, wherein the first object network includes a first object that is linked to a first plurality of pixel locations; (d) generating a second object network, wherein each object of the second object network belongs to the second object class, wherein the second object network includes a second object that is linked to a second plurality of pixel locations, wherein all of the objects of the second object network are linked to a third plurality of pixel locations, and wherein the third plurality of pixel locations includes the second plurality of pixel locations; (e) generating a third object network by imprinting each object of the first object network into the second object network, wherein the third object network includes a third object that is linked to a fourth plurality of pixel locations, wherein the fourth plurality of pixel locations is a subset of the third plurality of pixel locations and is calculated from the first plurality of pixel locations; (f) determining a characteristic of the third object by measuring the third object; and (g) displaying the third object on a computer display.
- **Claim 18.** A method comprising: in a specification mode specifying a first object class and a second object class; selecting process steps of an image analysis script that define a segmentation method that generates a first object network belonging to the first object class and a second object network belonging to the second object class; selecting an imprinting process step of the image analysis script by selecting the first object class and the second object class; selecting a measurement process step of the image analysis script by specifying characteristics of a target object; saving the image analysis script to a memory; acquiring pixel values that are associated with pixel locations of a digital image; and in an execution mode running the image analysis script, wherein the image analysis script: executes the process steps of the segmentation method and generates the first object network and the second object network, wherein the first object network includes a first object, and wherein the second object network includes a second object; imprints the first object network into the second object network such that a third object network is generated, wherein the third object network includes a third object; and determines a characteristic of the third object.
- **Claim 23 (computer-readable medium).** ...(a) generating a first plurality of objects by linking a first plurality of pixel locations on the digital image to the first plurality of objects, wherein the first plurality of objects forms a first object network; (b) linking each of the first plurality of objects to a first object class; (c) generating a second plurality of objects by linking a second plurality of the pixel locations to the second plurality of objects, wherein the second plurality of objects forms a second object network, wherein a third plurality of the pixel locations includes those pixel locations that belong both to the first plurality and to the second plurality of the pixel locations; (d) linking each of the second plurality of objects to a second object class; (e) generating a third object network by imprinting the first plurality of objects into the second plurality of objects such that the third plurality of pixel locations are relinked from objects of the second plurality of objects to objects of the third object network; and (f) determining a characteristic of an object of the third object network by measuring the object.
- **Claim 24.** Like claim 23, plus (a)/(b) specifying the classes, (c) acquiring pixel values, and imprinting "(h) ... such that objects of the first plurality of objects that become part of the third object network are linked to the third plurality of pixel locations, and objects of the second plurality of objects that become part of the third object network are not linked to the third plurality of pixel locations", then (i) measuring and (j) displaying.
- **Claim 35.** Specifying two classes; selecting a first process step "from a database of process steps" that "performs a first segmentation of objects of an object network into the first object class" and a second step for the second class; saving the script; acquiring pixels; running the script, which links pixels to both pluralities and "imprints the first plurality of objects into the second plurality of objects such that each of the third plurality of the pixel locations is unlinked from an object of the second plurality of objects and remains linked to an object of the first plurality of objects", and then measures.
- **Claim 38 (system, means-plus-function).** A display, and "means for measuring the object", where the means generates second and third object networks and "generates the first object network by imprinting the objects of the second object network into the objects of the third object network such that the fourth plurality of the pixel locations are unlinked from objects of the third object network and remain linked to objects of the second object network."

Dependent claims worth noting: **10** ("each object of the first object network having a higher probability of belonging to the first object class than belonging to the second object class"), 8–9 (imprint only objects that meet a condition, or a minimum share of pixels), 7 and 27 (split non-contiguous objects into contiguous ones), 4–6 and 29 (scale, rotate, translate before imprinting), 25–26 (different resolutions), 12 and 19 (restore and discard if the measurement fails), 14 and 32 (overwrite in memory), 15–17 and 33–34 (roads, roofs, nuclei).

### 1.2 US 7,801,361 B2: "Analyzing pixel data using image, thematic and object layers of a computer-implemented network structure"

| Item | Value | Source |
|---|---|---|
| Application | US 11/709,601, filed 2007-02-21; published as US 2008/0008349 A1 | [Google Patents](https://patents.google.com/patent/US7801361B2/en) |
| Related US data | Continuation-in-part of US 11/511,930, filed 2006-08-28 (which became US 7,873,223) | [USPTO PDF](https://patentimages.storage.googleapis.com/1b/5a/de/90cb1f1862cfbe/US7801361.pdf) |
| Foreign priority | DE 102 48 013.3 (2002-10-15); DE 10 2005 016 290.8 (2005-04-08) | same |
| Grant | 2010-09-21; inventors Gerd Binnig, Guenter Schmidt; assignee Definiens AG | same |
| PTA | **876 days**; no terminal disclaimer on the front page | same |
| Maintenance fees | 4th, 8th and 12th year all paid (12th on 2022-03-15) | Google legal events |
| Expiry | 2029-01-20 (= 2006-08-28 + 20 years + 876 days; matches Google) | computed and cross-checked |
| Family | **US only** for this patent. Its relatives are covered in 1.3. | Google Patents |

*Term caveat:* US 11/511,930 is itself a CIP of PCT/EP2006/061498 (filed 2006-04-10) and of US 10/687,477 (filed 2003-10-15) ([US7873223 page](https://patents.google.com/patent/US7873223B2/en)). US 7,801,361 refers only to 11/511,930, so the 20 years should run from 2006-08-28. If the benefit chain were held to reach the PCT, the expiry would be about 2028-09; if it reached 10/687,477, about 2026-03. An attorney should confirm which date applies.

**Independent claims** (verbatim from [Google Patents](https://patents.google.com/patent/US7801361B2/en); 1 and 25 in full, the others summarised):

- **Claim 1.** (a) specifying an object class, wherein a first plurality of objects belongs to the object class; (b) specifying a filter; (c) specifying process steps of a process hierarchy; (d) acquiring first pixel values [forming] a first image layer ...; (e) executing the process steps on a computer to generate the first plurality of objects by linking some of the first pixel locations to each of the first plurality of objects, wherein the first plurality of objects forms a first object network; (f) linking each of the first plurality of objects to the object class; (g) generating a second image layer by applying the filter to those first pixel values associated with first pixel locations that are linked to one of the first plurality of objects ...; (h) generating a second object network by linking some of the second pixel locations to each of a second plurality of objects; (i) determining a characteristic of one of the second plurality of objects by measuring the object; and (j) displaying the measured object on a graphical user interface of the computer.
- **Claim 8.** Filter the image into a second layer; a thematic layer assigns a thematic class to some pixels; a third image layer is made by processing only the pixels of that thematic class; an object network is generated from the third layer; measure; display.
- **Claim 14.** Filter; build an object network and a thematic layer from the filtered values; run a first algorithm only on the pixels linked to objects and a second algorithm only on the pixels of the thematic class; display.
- **Claim 20.** A thematic layer generated only at the pixels linked to an object; detect an edge where a pixel of thematic class 1 has a set number of neighbours of class 2; display.
- **Claim 25.** (a) acquiring a digital image comprising pixel values; (b) generating a thematic layer based on the pixel values, wherein a first thematic class and a plurality of other thematic classes are assigned to pixel locations associated with the pixel values; (c) generating an object network by segmenting pixel locations associated with contiguous thematic classes of the thematic layer; (d) assigning a first object class to objects of the object network that are linked to pixel locations assigned to the first thematic class; (e) modifying the thematic layer classes for those pixel locations that are linked to the objects of the first object class; (f) modifying the objects of the first object class by segmenting contiguous thematic classes of the thematic layer; (g) classifying the objects of the object network according to membership in a second object class; (h) measuring the objects that are assigned to the second object class; and (i) displaying the measured objects on a graphical user interface.
- **Claim 28.** (a) acquire an image; (b) segment contiguous regions into an object network; (c) classify the objects "according to a membership function of a first object class"; (d) assign a first thematic class to the pixels of those objects; (e) identify pixels "that are assigned the first thematic class and that fulfill a criterion of a second thematic class"; (f) resegment the first-class objects "by grouping contiguous pixel locations assigned to the first thematic class and the second thematic class"; (g) classify by a membership function of a second object class; (h) measure; (i) display.

### 1.3 Relatives and status in Europe and Brazil

| Document | Relation | Status | Source |
|---|---|---|---|
| US 7,873,223 B2 (11/511,930, filed 2006-08-28) | Parent of US7801361 | Google shows "Active, expires 2026-10-03", a date already past. Its own CIP chain (PCT filed 2006-04-10; 10/687,477 filed 2003-10-15) suggests it expired earlier. Either way it is **not in force today**. | [link](https://patents.google.com/patent/US7873223B2/en) |
| WO 2006/106152 A2 (PCT/EP2006/061498) | DE 10 2005 016 290 lineage | Ceased (the PCT phase is over). National phases: EP1866849 and the US only. | [link](https://patents.google.com/patent/WO2006106152A2/en) |
| EP 1 866 849 A2 | European phase of the PCT | **Refused** (refusal effective 2014-07-08) | [link](https://patents.google.com/patent/EP1866849A2/en) |
| EP 1 552 437 B1 (filed 2003-10-15; DE 10248013 lineage) | Granted 2008 for CH, DE, FR, GB, LI | The 20-year maximum ended **2023-10-15**: expired | [link](https://patents.google.com/patent/EP1552437B1/en) |
| US 7,146,380 B2 (10/687,477) | Same lineage | Expired 2025-01-04 | [link](https://patents.google.com/patent/US7146380B2/en) |
| DE 102 48 013.3 / DE 10 2005 016 290.8 | Priority applications | The 20-year maximum ended 2022-10-15 and 2025-04-08. DPMA register not checked. | dates from the US7801361 PDF |
| Brazil (INPI) | none | No BR member appears in any family listing above. INPI not searched directly. | Google family data |

**Conclusion on territory:** nothing from these two families is in force in Europe or Brazil. Both patents are US-only, so they bear only on making, using, selling or importing in the United States (including US users running Terraflux).

**Owner:** Definiens AG is still the recorded owner. Definiens later appears as Definiens GmbH and then AstraZeneca Computational Pathology GmbH (assignee names on later patents in the [Google assignee listing](https://patents.google.com/?assignee=Definiens&status=GRANT)). In 2010 Trimble bought the Earth Sciences business (eCognition) and *licensed* the Cognition Network Technology. No assignment of these patents to Trimble was found ([Global Venturing 2010](https://globalventuring.com/blog/2010/06/15/trimble-buys-some-of-definiens-assets/), [Newswire](https://www.newswire.com/news/trimble-acquires-definiens-earth-sciences-business-to-expand-its-156496)). The owner, and so the party who could sue, is therefore probably the Definiens/AstraZeneca entity, with Trimble as licensee. An attorney should confirm this in the USPTO assignment records.

---

## 2. Other Definiens / Trimble patents checked

Method: the Google Patents assignee listing for "Definiens", status GRANT (51 results); "Trimble Germany" (6 results, all optical or laser measurement); inventor searches (Binnig, Schmidt from 2004, Schaepe, Baatz); and a keyword search "object-based image analysis" with assignee Trimble (774 results, of which the top 100 are all positioning, survey, construction or BIM). **No Trimble or Trimble Germany patent on OBIA classification was found.** Relevant Definiens patents:

| Patent | Claim 1 (summary) | Status / expiry | Relevance to the five topics |
|---|---|---|---|
| **US 10,445,557 B2** (priority 2014-08-29; continuation of US 9,740,957) | Segment a *first portion* of an image into objects of a data network (objects with subobjects, linked to pixels); determine class characteristics; give each class a colour; generate **pixelwise descriptors** for which pixels belong to which class; apply them to each pixel of a *second portion* **without segmenting it**; give each pixel the colour of the class "most probably associated" with it; display the heat map. | **In force.** Google: expires 2035-05-31; 4th-year fee paid 2023. [link](https://patents.google.com/patent/US10445557B2/en) | Per-pixel "most probable class" is close to argmax, but the claim also needs a classifier learned from objects in another part of the image. It is relevant only to a *pipeline* (see 3.3). |
| US 9,740,957 B2 / US 9,805,248 B2 / US 10,474,874 B2 (same 2014 "pixelwise descriptors" family) | Not read (Google rate limit, Justia 403) | Probably in force until about 2034–2035 | **Attorney/next step:** read their claims; same risk as US10445557. |
| US 7,873,223 B2 | A class network with a class whose **membership function** defines a likelihood that an object belongs to it; a process step with a domain that designates the class (a process hierarchy); a link type; execution links data to objects. Claim 34 covers pixel values plus metadata. | **Expired / not in force** (Google date 2026-10-03 already past) | Covers membership-function classes and a process hierarchy. No longer a barrier. |
| US 6,832,002 B2 (Baatz et al., priority 1997) | Iterative segmentation that merges adjacent objects while the heterogeneity added by a merge stays within a tolerance, repeated until stable: **multiresolution segmentation** | Expired 2019-12-08 [link](https://patents.google.com/patent/US6832002B2/en) | Multiresolution segmentation is free to use. The EP counterpart EP0858051B1 (filed 1998) has also expired. |
| US 7,146,380 B2, US 7,467,159 B2, US 9,245,337 B2, EP 1 552 437 B1 | Semantic cognition network (data, class and process object networks); processing objects of data domain + class domain + algorithm; process hierarchy GUI; image mining | Expired 2025-01-04, 2024-06-25, 2024-09-02 and 2023-10-15 [380](https://patents.google.com/patent/US7146380B2/en), [159](https://patents.google.com/patent/US7467159B2/en), [337](https://patents.google.com/patent/US9245337B2/en) | Class hierarchy and process hierarchy: free |
| US 9,092,852 B2 | Organ detection: a second class is detected from its relation to a first detected organ, using a process hierarchy | **Lapsed**: the 8th-year fee was not paid (reminder 2023-03-20; lapse effective 2023-07-28) [link](https://patents.google.com/patent/US9092852B2/en) | A class found inside or relative to another: no barrier |
| US 8,594,410 B2 | Medical image-based biomarkers correlated with clinical endpoints over patient subgroups | In force until 2027-07-01 [link](https://patents.google.com/patent/US8594410B2/en) | Not relevant (medical, clinical correlation) |
| US 8,711,149 B2 | GUI: a table of image-analysis values per tissue slice; selecting a row shows the image | In force until 2032-09-08 [link](https://patents.google.com/patent/US8711149B2/en) | Not relevant |
| EP 0843864, EP 0870275, EP 0846306, EP 1025539 (1995–1998) | Early pattern classification, fractal networks | Expired (filings 1996–1998, so 20-year terms ended by 2018) | none |

Topic by topic: **arbitration by fuzzy membership**: only US8319793 dependent claim 10 and the conditional-imprinting claims come near it (section 3). **Class hierarchy, each class found inside its parent's region**: no in-force patent found (the CNT class-network patents have expired). **Above/below/between membership rules**: no in-force patent found (US7873223 and US7146380 have expired). **Multiresolution segmentation**: expired. **Nearest-neighbour classification of objects**: no Definiens or Trimble patent found.

---

## 3. Claim charts

What `objects.resolve` does: it takes N per-class membership rasters (0..1). For each cell it takes the argmax; the result must reach the least membership, otherwise the cell is unclassified; ties go by class order. Outputs: the class map, the winning membership, the doubt (second-best class and membership), the conflict count, and objects formed as connected cells of one class (optionally within given objects).

### 3.1 US 8,319,793: claim 1

| Element | objects.resolve | Present? |
|---|---|---|
| (a)(b) specify first and second object classes | It has N classes | Yes (trivial) |
| (c) **generate a first object network**, every object of the first class, linked to pixels | Inputs are membership rasters, not an object network. No set of class-1 objects exists before or inside the operation. | **Absent** (but see the pipeline note) |
| (d) **generate a second object network** of the second class whose objects cover a "third plurality" of pixels | Same as (c). Optional "given objects" are an unclassified segmentation, not class-2 objects. | **Absent** (arguable only for the "within given objects" mode if the given objects carry a class) |
| (e) **generate a third network by imprinting each object of the first network into the second**, so that the new object's pixels are a subset of the second network's pixels "calculated from the first plurality" | Each cell is decided symmetrically by its largest membership. No network takes precedence, nothing is "imprinted", and no pixels are relinked from existing objects. New objects come from connectivity in the class map. | **Absent** |
| (f) measure the third object | It counts conflicts and memberships; downstream statistics could measure objects | Could be present |
| (g) display | The app displays results | Could be present |

**Reading:** claim 1 is not met. The key elements (c)–(e) are absent: two pre-existing, class-specific object networks and the asymmetric "imprinting" of one into the other.

### 3.2 US 8,319,793: claims 18, 23, 24, 35, 38 and dependent claim 10

- **Claim 18:** needs a script with an "imprinting process step" chosen by selecting two classes, run on two object networks. `resolve` has no such step. **Not met.**
- **Claims 23, 24:** need two object pluralities, each linked to its class, overlapping in a "third plurality" of pixels, which are then *relinked from second-network objects to first-network objects* (claim 24: first-network objects take the overlap and second-network objects lose it). `resolve` relinks nothing between existing objects. **Not met.**
- **Claim 35:** two segmentation process steps from a "database of process steps", one per class, then the imprint unlinks the shared pixels from class-2 objects and keeps them linked to class-1 objects. **Not met.** This is the one most worth watching, because a Terraflux graph *does* chain operations picked from a catalogue (see 3.3).
- **Claim 38 (means-plus-function):** the "means" is limited to the imprinting structure in the specification and its equivalents. Per-cell argmax is a different algorithm. **Not met.**
- **Dependent claim 10** ("higher probability of belonging to the first ... class than ... the second") is the only claim that names membership comparison. It still needs all of claim 1, including two object networks and imprinting. **Not met.**

**Pipeline note (the one real risk for US8319793).** Suppose a user (or a Terraflux template) (1) segments the image twice, (2) gives class A memberships from the objects of segmentation 1 and class B memberships from the objects of segmentation 2, painted to cells, and (3) runs `resolve`. Where A beats B, cells move from B-objects to A-objects, and the connected output regions form a "third network". A patentee could argue that this is imprinting under the doctrine of equivalents, at least where A always wins (claim 10). Counter-arguments: the inputs are rasters, so nothing is "linked" to objects; the decision is symmetric per cell, not "each object of the first network" being imprinted; and the per-cell arbitration of overlapping hypotheses was published before 2009 (section 4). This risk ends when the patent does (most likely by about 2029-01-20).

### 3.3 US 7,801,361: claims 25 and 28 (relevant, though not to resolve alone)

**Claim 25**

| Element | objects.resolve | Present? |
|---|---|---|
| (a) acquire image | inputs are membership rasters derived from images | Yes, in a pipeline |
| (b) thematic layer: a first class and other classes assigned to pixels | **The class map is exactly this** | **Present** |
| (c) object network by segmenting contiguous thematic classes | **Objects joined as connected cells of one class** | **Present** |
| (d) give a first object class to objects on first-class pixels | The objects carry their class | **Present** |
| (e) **modify the thematic layer** at the pixels of first-class objects | `resolve` does not modify the class map afterwards | **Absent in resolve** |
| (f) **re-segment** the first-class objects from the modified thematic layer | none | **Absent** |
| (g) classify objects into a second object class | none inside `resolve` | **Absent** |
| (h)(i) measure, display | downstream | possibly |

**Claim 28:** (e) "identifying pixel locations that are assigned the first thematic class and that fulfill a criterion of a second thematic class" is close to the **conflict and doubt outputs**: cells won by class 1 where another class also passes the least membership. `resolve` performs neither (f), re-segmenting by grouping class-1 and class-2 pixels, nor (g), reclassifying by a second membership function. **Not met by resolve alone.**

**Reading:** `resolve` alone does not meet claims 25 or 28. However, it supplies (b)–(d) of claim 25 and (d)–(e) of claim 28. A Terraflux *workflow* that follows `resolve` with a step that edits the class map at the pixels of one class's objects (or regroups conflict cells with winning cells), forms objects again, reclassifies them into another class, measures them and shows the result could meet every element until about 2029-01-20.

**Claims 1, 8, 14, 20** need pixel-value filtering limited to pixels linked to objects or to a thematic class (or edge counting inside objects). `resolve` does not filter pixel values. **Not met.**

### 3.4 US 10,445,557: claim 1

| Element | objects.resolve | Present? |
|---|---|---|
| segment a first portion into data-network objects with subobjects | not done by resolve | **Absent** |
| class characteristics; a colour per class | colours in display only | partial |
| generate pixelwise descriptors (a classifier learned from the objects) | not in resolve; possibly a separate Terraflux classifier | **Absent in resolve** |
| apply them to each pixel of a second portion without segmenting it | resolve works per cell, but on memberships given to it | partial |
| each pixel gets the colour of its most probable class | **argmax class map, displayed in colour** | **Present** |
| display the heat map | yes | Present |

**Reading:** not met by `resolve` alone. Risk arises if Terraflux offers "train a pixel classifier from classified objects in area X, apply it per pixel in area Y without segmenting, show the argmax colour map". That workflow should be checked against this family until about 2035.

---

## 4. Prior art for per-cell or per-region arbitration of competing classes

All of these were published before 2009-04-17 (US8319793) unless marked otherwise. For US7801361 (effective date 2006-08-28 or earlier) the 2001–2006 items are the relevant ones.

1. **GeoAIDA (University of Hannover, TNT).**
   - Bückner, J., Müller, S., Pahl, M., Stahlhut, O., "Semantic interpretation of remote sensing data", *IAPRS* Vol. XXXIV Part 3 (ISPRS Comm. III symposium, Graz, **2002**), [PDF](https://www.isprs.org/proceedings/XXXIV/part3/papers/paper008.pdf). It says "The top-down-path can generate different hypothesis for one region. The bottom-up-step has to decide for an explicit interpretation for a region"; it mentions "a rejection class" for unclassified regions; the tasks of the bottom-up operator include "Adaption of the label images to the new obtained interpretation"; worked example: "the hypothesis industry has a quality value of 0,92 the competing hypothesis settlement has a value of 0,42. The analysis selects due to the quality values the interpretation industry", and the region is split into sub-regions of different classes.
   - Müller, S., Weis, M., Liedtke, C.-E., Pahl, M., "Automatic quality surveillance of GIS data with GeoAIDA", *ISPRS Archives* XXXIV-3/W8, Munich, **17–19 Sept 2003**, [PDF](https://www.pia.tum.de/isprs/pia03/pub/pia03_s8p2.pdf). It reads "in case of conflicting hypotheses the most probable decision is chosen"; the bottom-up step includes "Representation of the results in label images" and "Calculation of a quality description for the objects"; "If different hypotheses for one region are generated, the decision is made by the bottom-up-step according to ... quality values".
   - Gerke, M., "Scene analysis in urban areas using a knowledge-based interpretation system", *IAPRS* XXXIV Part 3 (**2002**), [PDF](https://www.isprs.org/proceedings/XXXIV/part3/papers/paper183.pdf). It reads "Possible ambiguities, occurring in the top-down process are also solved in the bottom-up phase" and "the solving of ambiguities, caused by competing hypotheses is done by the system."
   - Earlier: Bückner et al., "GeoAIDA – A knowledge based automatic image data analyser for remote sensing data", CIMA 2001, Bangor (cited in the papers above; I did not read the full text). GeoAIDA was released under the GPL (SourceForge/BerliOS "geoaida"; I could not open the project page to date the releases).
2. **InterIMAGE (PUC-Rio / INPE).**
   - Oliveira, D.A.B., Costa, G.A.O.P., Feitosa, R.Q., Castejon, E.F., Fonseca, L.M.G., "InterIMAGE: an open source knowledge based framework for automatic interpretation of remote sensing data", GEOBIA **2008**, Calgary, *IAPRS* XXXVIII-4/C1, [PDF](https://www.isprs.org/proceedings/XXXVIII/4-C1/Sessions/Session11/6715_Costa_proc_pap.pdf). It reads "the problem that different operators can generate different information for the same region ... is solved by the use of additional knowledge regarding the judgment of the competing interpretations"; "the validated child hypotheses are grouped and eventual spatial conflicts among them are resolved"; and a conflict resolved by "a simple rule that assigns different priority values for the respective concepts ... and chooses the hypothesis with higher priority". This is priority by class order, as in `resolve`'s tie rule. **August 2008, before the 2009 priority date.**
   - Costa, G.A.O.P., Feitosa, R.Q., Fonseca, L.M.G., Oliveira, D.A.B., Ferreira, R.S., Castejon, E.F., "Knowledge-based interpretation of remote sensing data with the InterIMAGE system: major characteristics and recent developments", GEOBIA **2010**, Ghent, *IAPRS* XXXVIII-4/C7, [PDF](https://www.isprs.org/proceedings/XXXVIII/4-C7/pdf/Costa.pdf). Top-down operators "may associate to the same geographic region different classes of objects. It is the task of the bottom-up operations to resolve the conflicts"; membership values are "used on the process of deciding among overlapping hypotheses"; the "Classify element ... is used to resolve spatial conflicts among competing hypotheses"; "if there is a partial spatial overlap ... the hypothesis with the lower membership value will not be completely discarded, only the overlapping part of it will be eliminated"; then "merge connected". *This is after 2009, so it is not prior art to US8319793, but it shows the open lineage (GeoAIDA to InterIMAGE) that `resolve` follows.*
   - Novack, T. et al., "Per block urban land use interpretation using optical VHR data and the knowledge-based system InterIMAGE", GEOBIA 2010, [PDF](https://www.isprs.org/proceedings/XXXVIII/4-C7/pdf/Novack_Abstract.pdf). Leftover conflicts "are solved either by the supervised definition of priority for the classes or by the competition of membership values given by user-defined fuzzy membership functions" (2010).
3. **eCognition's own published outputs (best class, second best, ambiguity).** Schiewe, J., Gähler, M., "Modelling uncertainty in high resolution remotely sensed scenes using a fuzzy logic approach", OBIA **2006**, Salzburg, *IAPRS* XXXVI-4/C42, [PDF](https://www.isprs.org/proceedings/XXXVI/4-C42/Papers/07_Adaption%20and%20further%20development%20I/OBIA2006_Schiewe_Gaehler.pdf). It documents eCognition's "'Classification stability': For each pixel the difference between the largest and second largest membership values" and the "'Best classification result' ... the corresponding largest membership value for each pixel". This is prior art for the winning-membership and doubt outputs. Also Benz, U.C. et al., "Multi-resolution, object-oriented fuzzy analysis of remote sensing data for GIS-ready information", *ISPRS J. Photogramm. Remote Sens.* 58 (**2004**) 239–258, doi:10.1016/j.isprsjprs.2003.10.002 (bibliographic record confirmed; full text not read here).
4. **Classic per-pixel arbitration (bibliographic knowledge, not re-read here):** Chow, C.K., "On optimum recognition error and reject tradeoff", *IEEE Trans. Inf. Theory* 16(1):41–46, **1970**: argmax with a reject threshold, i.e. "least membership, else unclassified". Wang, F., "Fuzzy supervised classification of remote sensing images", *IEEE TGRS* 28(2):194–201, **1990**: per-pixel fuzzy memberships, hardened by the maximum. Burrough, P.A., van Gaans, P.F.M., Hootsmans, R., "Continuous classification in soil survey: spatial correlation, confusion and boundaries", *Geoderma* 77:115–135, **1997**, doi:10.1016/S0016-7061(97)00018-9: one membership map per class plus a "confusion index" built from the largest and second-largest memberships, i.e. the doubt output. Connected-component labelling of a class map: Rosenfeld & Pfaltz, *J. ACM* 13(4), **1966**.

Every output of `resolve` (argmax, threshold to unclassified, priority tie-break, best membership, second best, conflict detection, connected objects) appears in literature published before both priority dates.

---

## 5. Conclusion (technical reading)

**Patents that could be read on, and how strongly:**
- **US 8,319,793**: **no independent claim (1, 18, 23, 24, 35, 38) is met by `objects.resolve`**. The core elements are missing: two class-specific object networks as inputs, and imprinting one into the other (relinking overlap pixels from existing objects of network 2 to those of network 1). Residual risk is limited to the doctrine of equivalents in a pipeline that paints object-based memberships from two different segmentations into `resolve`. The prior art in section 4 weakens such an argument. Because of the terminal disclaimer, the patent most likely ends **about 2029-01-20, not 2031-09-28**.
- **US 7,801,361**: `resolve` alone does not meet any claim. But it supplies most of claim 25 ((b)–(d)) and part of claim 28 ((d)–(e)). The risk is a **downstream workflow**: edit the class map at one class's objects, re-segment, reclassify into a second class, measure and display. In force until about 2029-01-20.
- **US 10,445,557** (and probably US9740957, US9805248, US10474874): not met by `resolve`. Risk applies only to a "learn a pixel classifier from objects in one part, apply it per pixel elsewhere without segmenting, display the argmax colours" workflow, until about 2035.
- **Clearly not a barrier:** multiresolution segmentation (US6832002, expired), class or process hierarchies and membership-function classes (US7873223, US7146380, US7467159, EP1552437, all expired), organ-relative classes (US9092852, lapsed), medical and GUI patents (US8594410, US8711149: different subject matter).
- **Europe and Brazil:** no member of these families is in force. All the patents that matter are US-only.

**Design choices that keep `objects.resolve` clear** (until 2029, plus the 2035 caveat):
1. Keep the inputs as **per-class membership rasters**. Do not take two object networks or two segmentation levels as inputs and output a third network made by overwriting one with the other.
2. Keep the decision **symmetric per cell** (largest membership wins; class order only breaks exact ties). Do not add a mode where "class A objects overwrite class B objects" or "level 1 is printed into level 2". That is imprinting.
3. **Never relink cells of existing objects.** Output objects should always be *new*, formed from connectivity of the class map. In the "within given objects" mode, the given segmentation should stay unclassified input geometry that is only intersected and never modified, and should not be described as "imprinting".
4. Do not ship (in `resolve` or as a ready-made template right after it) a loop that **edits the class map at one class's objects, re-segments those cells and reclassifies them into a second class** (claim 25), or that **groups conflict/doubt cells with winning cells and reclassifies them** (claim 28). Leave the conflict and doubt maps as diagnostic outputs.
5. Do not add rollback such as "restore the previous objects if the measurement fails" around a merge of two networks (claims 12 and 19), or scale/rotate/translate objects between two images before merging (claims 4–6 and 29).
6. Do not filter pixel values only inside objects or a thematic class as part of this flow (US7801361 claims 1, 8 and 14).
7. In the docs and code, cite GeoAIDA (2002/2003) and InterIMAGE (2008/2010) as the sources of the method, and describe it as "arbitration of competing class hypotheses", not "imprinting".
8. Before offering a "train a pixel classifier from objects, apply it unsegmented elsewhere, show the argmax map" workflow, check the 2014 pixelwise-descriptor family (US10445557 and siblings).

## 6. What a patent attorney should still confirm

1. **The terminal disclaimer in US 8,319,793:** which patent it names (most likely US 7,801,361), the exact disclaimed date (with or without the reference patent's PTA), and whether common ownership has continued, since the disclaimer makes it enforceable only while that holds. All of this is in the Patent Center file wrapper for 12/386,380.
2. **The term of US 7,801,361:** whether it runs from 2006-08-28 (giving 2029-01-20) or from an earlier link in the benefit chain (the PCT of 2006-04-10, or 10/687,477 of 2003-10-15).
3. **Current owner and licensees:** the USPTO assignment records (Definiens AG to Definiens GmbH to AstraZeneca Computational Pathology GmbH?) and whether Trimble holds an exclusive licence with the right to sue.
4. Claim construction of "object network", "imprinting" and "linked" (in the specification of US8319793), and whether the doctrine of equivalents could reach a symmetric per-cell argmax over membership rasters.
5. The claims of US 9,740,957, US 9,805,248 and US 10,474,874, and any Definiens/AstraZeneca or Trimble applications still pending (published applications were not searched systematically).
6. Searches in DPMA, EPO (Espacenet/Register) and INPI (Brazil) to confirm that no national counterparts exist. I relied on Google Patents family data.
7. Exposure for open-source distribution (inducement or contributory infringement for US users), and whether to keep a dated record of this design rationale.

---

## 7. Checked against Terraflux (2026-11-19)

- `objects.resolve` (`crates/ops/src/objects/resolve.rs`) takes only a stack of membership rasters (and an optional
  region map); it never takes two object networks or two levels. Its objects are always new, made from the class map:
  one per connected area (`connected`), one per class (`global`) or one per cell (`none`). Ties go by `order`, the
  decision is otherwise symmetric per cell. Its description cites GeoAIDA and InterIMAGE.
- The class tree (`crates/app/src/class_tree.rs`, the template "Each class its own way"): every recipe gives a
  membership map as a cell-wise product with `@region` (thresholds of indices, heights, or the classes of the same
  objects classified by samples); no branch segments again inside its mother's region, and nothing after the
  `objects.resolve` edits the class map, re-segments and reclassifies (US7801361 claims 25 and 28).
- The words "imprint"/"imprinting" appear nowhere in the code or the site.
- Not offered: a pixel classifier learned from objects in one area and applied unsegmented in another with the argmax
  colours (the US10445557 family, to about 2035).
