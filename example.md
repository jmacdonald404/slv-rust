{{Merge|Authentication Flow}}

==Introduction==


This page documents the log-in protocol for Second life as it  exists currently. It will eventually be replaced by the login portion of the [[Open Grid Protocol|Open Grid Protocol (OGP)]]. 

The purpose of this document and others like it is to give a detailed description of the Second Life protocols so that programmers can refer to it when implementing their own Second Life viewer without referring to or using the source code of either the Linden Lab viewer, or the libsecond life viewer. It also allows internal developers at Linden Lab to check their understanding of the current log-in protocol as they develop the new version described in the SLGOGP. 

Working examples of this protocol are found on the links in the  [[Example_protocol_code| Example Protocol Code]] page

== Conventions ==

On this page the following conventions are used:

:"in quotes is a literal string"
:[ represents a list of choices | separated by a vertical bar ]
:< represents a value which can be substituted by an appropriate string described inside > 
:'represents a string that must be quoted, but how will be implementation specific' for example in LSL "" denotes a string and " will be used in place of the single quote (').
:+ means to concatenate the two parts, though how will be implementation specific, for example + can be used to concatenate strings.
==Log-in==
The current Second Life log-in requires the viewer to send an [http://www.xmlrpc.com/#whatIsXmlrpc http/1.1 XML-RPC] message with [[#Input parameters|specific input parameters]] to a Second Life log-in URL and parse the response for further processing. The URL is specified by using "-loginuri" in the command line parameters, as described in the [[Client_parameters|command line parameters]] page on the Second Life wiki. 
===Authentication Flow===
This is a simplified version of the [[Authentication Flow|authentication  flow]], showing the major steps in the process.
[[Image:Second LIfe Login UML2.png|thumb|UML sequence diagram]]
Note: Step 5 through Step 8 establish presence after login and are documented in the [[Establishing_Avatar_Presence]] page.


;Step 1
:''Viewer'' ------------> ''Login Server'' {XML-RPC)

;Step 2
:''Login Server'' ------> ''Database''

;Step 3
:''Login Server'' -----> ''SIM in the Grid''

;Step 4
:''Login server'' ------> ''Viewer'' (XML-RPC response)

;Step 5
:''Viewer''  ------------> ''Simulator'' UseCirucuitCode (UDP expects ack)

;Step 6
:''Viewer''  ------------> ''Simulator'' CompleteAgentMovement (UDP)

;Step 7
:''Viewer''  ------------> ''Simulator'' AgentUpdate (UDP expects ack)

;Step 8
:''Simulator'' ---------> ''Viewer'' AgentMovementComplete (UDP)

===XML-RPC call===
Note: This section details Step 1 and Step 4 of the [[#Authentication Flow]].
====Input parameters====
=====Required parameters=====
The following explains the standard key value pairs, with an explanation of "options" at the end:

:• '''"first": <first>''' 
::''first name of the avatar.''
:• '''"last": <last>''' 
::''last name of the avatar.''
:• '''"passwd": '$1$'''' + '''<passwd_md5>''' 
::''the avatar password encrypted using MD5 encryption.''
:• '''"start": ["home" | "last" | <specific location> ]'''
::''attempt to log in to this sim, though if it is full or not available, or the agent is not allowed, another sim will be selected by the grid from its list of choices. "Home" means home location. If home is filled then the grid will try last. If last is filled and home is set, the grid will try home. In either case the last choice will be from a list of [[telehubs]]. For a log in to a specific location the format is "uri:<existing region name>&<x>&<y>&<z>".''
:• '''"channel": <channel name>'''  
::''the name of the client. Used to differentiate official viewers from third party clients.''
:• '''"version": <version string>''' 
::''version number of the client.''
:• '''"platform": ["Lin" | "Mac" | "Win"]''' 
::''the platform of the client.''
:• '''"mac": <MAC Address>''' 
::''the MAC address associated with the client's computer.''
:• '''"options": [optional_login]''' 
::''optional array of character strings. ([[Current_login_protocols#Optional Parameters|See Optional Parameters below]])''
:•''' "id0": "00000000-0000-0000-0000-000000000000"''' 
::''hardware hash (based on the serial number of the first hard drive in Windows) used for uniquely identifying computers.''
:• '''"agree_to_tos": ["true" | "false" | ""]'''
::''whether or not the user has agreed to the Terms of Service.''
:• '''"read_critical": ["true" | "false" | ""]'''
::''whether or not the user has read important messages such as Terms of Service updates.''
:• '''"viewer_digest": "00000000-0000-0000-0000-000000000000"''' 
::''MD5 hash of the viewer executable, only relevant when the channel is set to an official Second Life viewer.''

=====Optional Parameters=====
Zero or more of the  following character strings may appear in any order in the options array:
:[ "inventory-root" | "inventory-skeleton" | "inventory-lib-root" |
:"inventory-lib-owner" | "inventory-skel-lib" | "gestures" | "event_categories" |
:"event_notifications" | "classified_categories" | "buddy-list" | "ui-config" | 
:"login-flags" | "global-textures" | "adult_compliant" ]
''See [[#Optional Response|Optional Response]] for descriptions of information returned from these options''

The 'adult_compliant' optional parameter is used by a viewer to indicate that it understands the 'A' or Adult region access level in addition to the standard 'PG' and 'M' settings.   See the [http://svn.secondlife.com/trac/linden/changeset/2573 code in Snowglobe] that adds this value during login.

You must add this to your own viewers if it is based on the Second Life 1.23 source code.   Snowglobe and later viewers will already have this flag indicating they properly process adult access levels.

===Response===
The valid (non-error) value returned by the login call is in standard xmlrpc name, value format. The most important of these is the [[Current_login_protocols#Login_Seed-Capability|"Login Seed-Capability"]] discussed below (not to be confused with the new "seed capability" for the new login procedure):

:Note: all return values are in "name: value" format as used in [http://docs.python.org/lib/typesmapping.html Python 2.5 dictionaries] unless otherwise noted.
:Note: [[Login_Return_Values|A complete dump of return values]] was moved to another page for clarity.

====Required Response====

:'''last_name'''
::''last name of avatar''--identical to name given in input parameters
:{ 'last_name': <last_name>}

:'''sim_ip'''
::''ip address used to establish UDP connection with startup simulator''
:{ 'sim_ip':<ip_num> }

:'''start_location'''
::''Result of 'start' parameter as specified in input parameters''
:{'start_location':"home" | "last" | <specific location> }

:'''seconds_since_epoch'''
::''seconds...''    '''clarification needed'''
:'seconds_since_epoch':<int>

:'''message'''
::''message of the day from login''
:{ 'message': <string> }

:'''first_name'''
::''first name of avatar''--identical to name given in input parameters
:[ 'first_name': <first_name> }

:'''circuit_code'''
::''used to validate UDP connection with login simulator''
:{ 'circuit_code': <int> }

:'''sim_port'''
:''port used to establish UDP connection with login simulator''
::{ 'sim_port':<int> }

:'''secure_session_id'''
:''secure token for this login session--never used in UDP packets (unknown if this is unique per login or unique per simulator)'' '''clarification needed'''
::'secure_session_id': <uuid>

:'''look_at'''
:''initial camera direction (3D vector) of avatar''
::{ 'look_at ': r + <real>, r + <real>, r + <real> }

:'''agent_id'''
::''permanent UUID of avatar''
:{ 'agent_id': <uuid> }

:'''inventory_host'''
::''name of database used for inventory''
:{ 'inventory_host': <name> }

:'''region_y'''
::''The 'y' grid coordinate of the region''
:{ 'region_y': <int> }

:'''region_x'''
::''The 'x' grid coordinate of the region''
:{ 'region_x': <int> }

:'''seed_capability'''
::''[Capabilities|Capability]] that provides access to various capabilities as described in [[Current_Sim_Capabilities]], the most import of these is the EventQueueGet''
:{ 'seed_capability': <[[Capabilities|capability]]> }

:'''agent_access: M'''
::''authorization  information about access to main/mature grid as opposed to teen grid'' '''clarification needed'''
:{ 'agent_access': <'M'|'T'> }

:'''session_id'''
::"UUID for  current session with simulator. used in UDP message passing'' '''clarification needed'''
:{'session_id': <uuid> }

:'''login'''
::''...'' '''clarification needed'''
:{ 'login': 'true' }

====Optional Response====

:'''inventory-root''' 
::''UUID of the agent’s root inventory folder.''
:{ 'inventory-root': [{'folder_id': <uuid>}] }  

:'''inventory-skeleton''' 
::''Initial list of folders in agent’s inventory. Returned as an array of five-entry dictionaries. Each dictionary element describes a folder with its name, version, type, its UUID, and the UUID of the containing folder.''
:{'inventory-skeleton': [{'parent_id': <uuid>, 'version': <int>, 'name': <name>, 'type_default': <int>, 'folder_id': <uuid>},  .... ]}

:'''inventory-lib-root''' 
::''folder_id of library root inventory folder.''
:{ 'inventory-lib-root': [{'folder_id': <uuid>}] }

:'''inventory-lib-owner''' 
::''agent_id of owner for inventory lib. Used to establish common inventory library for all avatars in Second Life''  
'''Note''': Not the same as the agent_id in the [[#Required_Response |required response]] section
:{ 'inventory-lib-owner': [{'agent_id': <uuid>}] }

:'''inventory-skel-lib''' 
::''Initial list of folders in agent’s inventory. Returned as an array of five element dictionaires. Each dictionary describes a folder with its name, its UUID, the UUID of the containing folder, its type, its version.''
:{'inventory-skeleton':  [{'parent_id': <uuid>, 'version': <int>, 'name': <name>, 'type_default': <int>, 'folder_id': <uuid>},... ]}

:'''gestures''' 
::''List of active gestures. An array of two element dictionaries with the inventory item uuid and the asset uuid.''
:{ 'gestures':  [{'item_id': <uuid>, 'asset_id': <uuid>},...] }

:'''event_categories'''  
::''List of different event categories, mapping category id (an integer) to a category name. Returned as an array of two element dictionaries. Each dictionary describes a category’s id and it’s name.''
:{ 'event_categories': [{'category_id': <int>, 'category_name': <name>},...] }

:'''event_notifications''' 
::''List of events for which the agent has pending notifications. An array of eight-element dictionaries containing: event_id, event_name, event_desc, event_date, grid_x, grid_y, x_region, y_region.''
:{'events': [{"event_id":<uuid>, "event_name"<name>,"event_desc":<string>, "event_date":<date>, "grid_x":<float>, "grid_y":<float>, "x_region":<float>, "y_region":<float>}, ...]}
                  
:'''classified_categories"''' 
::''List of classifieds categories, mapping category id (an integer) to a category. Returned as an array of two element dictionaries with a category’s id and it’s name.''
:{ 'event_categories': [{'category_id': <int>, 'category_name': <name>},...] }
           
:'''buddy-list'''
::''List of friends with granted and given rights masks. Returned as an array  of three-element dictionaries with riend’s agent id, granted rights mask, given rights mask.''
:{ 'buddy-list':[{'buddy_id': <uuid>', 'buddy_rights_given': <int>, 'buddy_rights_has': <int>}, ....] }
            
:'''ui-config''' 
::''list of UI enabled/disabled states, currently: allow_first_life ('Y' or 'N') for teens.''  
:{ 'ui-config': {'allow_first_life': if allow first life} }

:'''login-flags'''  
::''Several flags about the state of the agent.''
:{ 'login-flags': {'stipend_since_login': <'Y'|'N'>,  'ever_logged_in': <'Y'|'N'>, 'gendered': <'Y'|'N'>, 'daylight_savings': <'Y'|'N'>} }

:'''global-textures''' 
::''The asset ids of several global textures.''
:{ 'global-textures': {'sun_texture_id': <uuid>, 'moon_texture_id': <uuid>, 'cloud_texture_id': <uuid>} }

:'''adult_compliant''' 
::''No special data returned, but this parameter indicates the viewer understands the 'Adult' region access level''

==Login Seed-Capability==

The current Login Seed-Capability is a [[Capabilities|Capability]] associated with the login sim. It should not be confused with the [[Seed-Capability|Seed-Capability]] proposed for the new protocols.

See [[Current_Sim_Capabilities|Current Sim Capabilities]] for more info.

==External Links==

[[Category: AW Groupies]]
